import { methods, parseCall } from "@shared/contract.ts";
// The IPC allowlist: the renderer can call only listed methods, with params that satisfy their schemas.
import { describe, expect, it } from "vitest";

describe("IPC contract", () => {
  it("refuses methods that are not listed", () => {
    for (const method of [
      "shell.openExternal",
      "fs.readFile",
      "__proto__",
      "constructor",
      "",
      42,
      null,
    ]) {
      expect(() => parseCall(method, undefined)).toThrow("Unknown method");
    }
  });

  it("refuses malformed params", () => {
    expect(() =>
      parseCall("profile.create", { name: "", nodeUrl: "http://localhost:8080" }),
    ).toThrow("Enter a name");
    expect(() =>
      parseCall("profile.create", { name: "Ana", nodeUrl: "file:///etc/passwd" }),
    ).toThrow();
    expect(() =>
      parseCall("profile.create", { name: "Ana", nodeUrl: "javascript:alert(1)" }),
    ).toThrow();
    expect(() => parseCall("lobby.join", { invite: "https://evil.example" })).toThrow(
      "This is not a Tecpan invite",
    );
    expect(() => parseCall("lobby.get", { lobby: "0x1234" })).toThrow("Expected hex");
    expect(() => parseCall("vault.details", { vault: "0xZZ" })).toThrow();
    expect(() => parseCall("settings.update", { relays: ["/ip4/1.2.3.4/tcp/1"] })).toThrow("/p2p/");
    expect(() => parseCall("clipboard.write", { text: "x".repeat(9000) })).toThrow();
    expect(() =>
      parseCall("lobby.create", { name: "V", threshold: 2, viewingThreshold: 1 }),
    ).toThrow();
  });

  it("refuses params on methods that take none", () => {
    expect(() => parseCall("app.state", { anything: true })).toThrow();
  });

  it("accepts well-formed calls and strips nothing it needs", () => {
    expect(
      parseCall("profile.create", { name: " Ana ", nodeUrl: "http://localhost:8080" }).params,
    ).toEqual({
      name: "Ana",
      nodeUrl: "http://localhost:8080",
    });
    expect(parseCall("settings.update", { technicalDetails: true }).params).toEqual({
      technicalDetails: true,
    });
    expect(
      parseCall("settings.update", {
        relays: [
          "/dns4/relay.example.com/udp/4001/quic-v1/p2p/12D3KooWHwqDpuKQk5eTnsiTzCC2BHeQH3bM6JihH416RaEGHhSc",
        ],
      }).params,
    ).toBeTruthy();
  });

  it("assigns every method to exactly one process", () => {
    for (const spec of Object.values(methods)) expect(["main", "engine"]).toContain(spec.side);
  });
});
