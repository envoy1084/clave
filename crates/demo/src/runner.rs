//! Drives the story: types each command into its terminal, waits for it, reads values from the daemons, and
//! advances. The UI (or the headless loop) calls [`Runner::tick`] often and [`Runner::go`] to start a step.

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::{
    pane::Pane,
    script::{Act, Capture, Step, Who},
};

/// Where the story stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// Showing the next step's narration; [`Runner::go`] runs it.
    Ready,
    Running,
    /// A command failed; retry or skip the step.
    Failed(String),
    Finished,
}

/// What the runner needs to call `clave` itself.
pub(crate) struct Tools {
    pub(crate) clave: PathBuf,
    pub(crate) env: Vec<(String, String)>,
}

enum Key {
    Bytes(Who, Vec<u8>),
    Submit(Who),
}

pub(crate) struct Runner {
    pub(crate) panes: [Pane; 2],
    pub(crate) steps: Vec<Step>,
    pub(crate) step: usize,
    pub(crate) state: State,
    pub(crate) focus: Who,
    /// What the runner is doing right now, for the status line.
    pub(crate) activity: String,
    act: usize,
    started: bool,
    submitted: bool,
    keys: VecDeque<(Key, Duration)>,
    next_key: Instant,
    vars: HashMap<&'static str, String>,
    capture: Option<Receiver<Result<String, String>>>,
    tools: Tools,
    headless: bool,
}

impl Runner {
    pub(crate) fn new(panes: [Pane; 2], steps: Vec<Step>, tools: Tools, headless: bool) -> Self {
        Self {
            panes,
            steps,
            step: 0,
            state: State::Ready,
            focus: Who::Alice,
            activity: String::new(),
            act: 0,
            started: false,
            submitted: false,
            keys: VecDeque::new(),
            next_key: Instant::now(),
            vars: HashMap::new(),
            capture: None,
            tools,
            headless,
        }
    }

    pub(crate) fn tools_env(&self) -> Vec<(String, String)> {
        self.tools.env.clone()
    }

    pub(crate) fn pane(&mut self, who: Who) -> &mut Pane {
        &mut self.panes[who.index()]
    }

    /// Both shells printed their first prompt.
    pub(crate) fn shells_ready(&self) -> bool {
        self.panes.iter().all(|p| p.ready)
    }

    /// Runs the current step (from its first action, also after a failure).
    pub(crate) fn go(&mut self) {
        if matches!(self.state, State::Ready | State::Failed(_)) && self.shells_ready() {
            self.state = State::Running;
            self.act = 0;
            self.started = false;
        }
    }

    /// Skips a failed step.
    pub(crate) fn skip(&mut self) {
        if matches!(self.state, State::Failed(_)) {
            self.advance_step();
        }
    }

    /// The terminal whose command is running most recently, if any.
    pub(crate) fn busy_pane(&self) -> Option<Who> {
        if self.panes[self.focus.index()].busy {
            return Some(self.focus);
        }
        [Who::Alice, Who::Bob]
            .into_iter()
            .find(|w| self.panes[w.index()].busy)
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        for pane in &mut self.panes {
            pane.poll();
        }
        if self.state != State::Running {
            return;
        }
        // Typing: one key per tick at the scheduled pace.
        while let Some((_, delay)) = self.keys.front() {
            if now < self.next_key {
                return;
            }
            let delay = *delay;
            let Some((key, _)) = self.keys.pop_front() else {
                break;
            };
            match key {
                Key::Bytes(who, bytes) => self.pane(who).send(&bytes),
                Key::Submit(who) => {
                    self.pane(who).submit();
                    self.submitted = true;
                }
            }
            self.next_key = now + delay;
            if !delay.is_zero() {
                return;
            }
        }
        for _ in 0..8 {
            if !self.step_act() {
                return;
            }
        }
    }

    /// Starts or checks the current action; returns whether to look at the next one in the same tick.
    fn step_act(&mut self) -> bool {
        let Some(act) = self
            .steps
            .get(self.step)
            .and_then(|s| s.acts.get(self.act))
            .cloned()
        else {
            self.advance_step();
            return false;
        };
        if !self.started {
            return self.start(&act);
        }
        let done = match act {
            Act::Run { who, wait, .. } => {
                self.submitted && self.keys.is_empty() && !(wait && self.panes[who.index()].busy)
            }
            Act::Wait(who) => !self.panes[who.index()].busy,
            Act::Capture(capture) => match self.capture.as_ref().map(Receiver::try_recv) {
                Some(Ok(Ok(value))) => {
                    self.vars.insert(capture.key(), value);
                    self.capture = None;
                    true
                }
                Some(Ok(Err(error))) => {
                    self.capture = None;
                    self.state = State::Failed(error);
                    return false;
                }
                _ => false,
            },
        };
        if !done {
            return false;
        }
        let waited = match act {
            Act::Run {
                who, wait: true, ..
            }
            | Act::Wait(who) => Some(who),
            _ => None,
        };
        if let Some(who) = waited
            && let Some(code) = self.panes[who.index()].last.filter(|c| *c != 0)
        {
            let title = self.panes[who.index()].title;
            self.state = State::Failed(format!("{title}'s command failed (exit {code})"));
            return false;
        }
        self.act += 1;
        self.started = false;
        true
    }

    fn start(&mut self, act: &Act) -> bool {
        match act {
            Act::Run { who, command, .. } => {
                if self.panes[who.index()].busy {
                    return false;
                }
                self.focus = *who;
                self.submitted = false;
                self.activity = format!("{} runs a command", self.panes[who.index()].title);
                self.type_command(*who, command);
            }
            Act::Wait(who) => {
                self.activity = format!("waiting for {}", self.panes[who.index()].title);
            }
            Act::Capture(capture) => {
                "reading from the daemon".clone_into(&mut self.activity);
                let (tx, rx) = mpsc::channel();
                let (clave, env, capture) =
                    (self.tools.clave.clone(), self.tools.env.clone(), *capture);
                std::thread::spawn(move || {
                    let _ = tx.send(read_value(&clave, &env, capture));
                });
                self.capture = Some(rx);
            }
        }
        self.started = true;
        true
    }

    /// Queues `command` as keystrokes: literal text typed at a readable pace, `{name}` values pasted at once.
    fn type_command(&mut self, who: Who, command: &str) {
        let per_key = if self.headless {
            Duration::ZERO
        } else {
            Duration::from_millis(22)
        };
        let mut rest = command;
        while !rest.is_empty() {
            if let Some(open) = rest.find('{') {
                for c in rest[..open].chars() {
                    self.keys
                        .push_back((Key::Bytes(who, c.to_string().into_bytes()), per_key));
                }
                let close = rest[open..].find('}').map_or(rest.len(), |i| open + i);
                let name = &rest[open + 1..close];
                let value = match name {
                    "yes" if self.headless => " -y".to_owned(),
                    "yes" => String::new(),
                    _ => self.vars.get(name).cloned().unwrap_or_default(),
                };
                if !value.is_empty() {
                    self.keys
                        .push_back((Key::Bytes(who, value.into_bytes()), per_key));
                }
                rest = rest.get(close + 1..).unwrap_or_default();
            } else {
                for c in rest.chars() {
                    self.keys
                        .push_back((Key::Bytes(who, c.to_string().into_bytes()), per_key));
                }
                rest = "";
            }
        }
        let pause = if self.headless {
            Duration::ZERO
        } else {
            Duration::from_millis(250)
        };
        if let Some(last) = self.keys.back_mut() {
            last.1 = pause;
        }
        self.keys.push_back((Key::Submit(who), Duration::ZERO));
    }

    fn advance_step(&mut self) {
        self.step += 1;
        self.act = 0;
        self.started = false;
        self.activity.clear();
        self.state = if self.step >= self.steps.len() {
            State::Finished
        } else {
            State::Ready
        };
    }
}

/// Reads a captured value with `clave --json`, retrying for up to two minutes until it appears.
fn read_value(
    clave: &PathBuf,
    env: &[(String, String)],
    capture: Capture,
) -> Result<String, String> {
    let (profile, args, pointer): (&str, &[&str], &str) = match capture {
        Capture::Invite => ("alice", &["lobby", "list"], "/0/invite"),
        Capture::BobAccount => ("bob", &["status"], "/profile/account"),
        Capture::Proposal => ("alice", &["proposals"], "/0/id"),
        Capture::Token => ("alice", &["token", "list"], "/0/address"),
    };
    let deadline = Instant::now() + Duration::from_mins(2);
    loop {
        let output = Command::new(clave)
            .args(["--json", "-P", profile])
            .args(args)
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .output()
            .map_err(|e| format!("clave: {e}"))?;
        let value = serde_json::from_slice::<Value>(&output.stdout)
            .ok()
            .and_then(|v| {
                v.pointer(pointer)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .filter(|v| !v.is_empty());
        if let Some(value) = value {
            return Ok(match capture {
                Capture::Proposal => value.trim_start_matches("0x").chars().take(6).collect(),
                _ => value,
            });
        }
        if Instant::now() > deadline {
            return Err(format!(
                "could not read the {} from the daemon",
                capture.key()
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
