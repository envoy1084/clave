import { type FormEvent, useState } from "react";

import { useQueryClient } from "@tanstack/react-query";

import { XIcon } from "lucide-react";

import { CopyValue } from "@/components/copy";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { PageHeader } from "@/features/vault/page-header";
import { useContacts } from "@/features/vault/use-vault";
import { call } from "@/lib/ipc";
import { keys } from "@/lib/state";

const ADDRESS = /^0x[0-9a-fA-F]{64}$/u;

/** Labelled counterparties. Stored in this profile only; never shared with the keepers or other members. */
export function Contacts() {
  const contacts = useContacts();
  const client = useQueryClient();
  const [address, setAddress] = useState("");
  const [label, setLabel] = useState("");
  const save = async (event: FormEvent) => {
    event.preventDefault();
    client.setQueryData(
      keys.contacts,
      await call("contacts.put", { address: address.trim(), label: label.trim() }),
    );
    setAddress("");
    setLabel("");
  };
  const remove = async (target: string) =>
    client.setQueryData(keys.contacts, await call("contacts.remove", { address: target }));
  return (
    <>
      <PageHeader
        title="Address book"
        description="Labels for Aztec addresses. Local to this profile, never sent anywhere."
      />
      <form
        onSubmit={(e) => void save(e)}
        className="mb-5 grid grid-cols-[minmax(0,1fr)_220px_auto] gap-2"
      >
        <Input
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          placeholder="0x… address"
          className="h-10 font-mono text-[0.8rem]"
          aria-label="Address"
        />
        <Input
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          placeholder="Label"
          maxLength={48}
          className="h-10"
          aria-label="Label"
        />
        <Button
          type="submit"
          className="h-10"
          disabled={!ADDRESS.test(address.trim()) || !label.trim()}
        >
          Save
        </Button>
      </form>
      <section className="overflow-hidden rounded-2xl bg-card shadow-[0_0_0_1px_var(--border)]">
        {!contacts.data || contacts.data.length === 0 ? (
          <p className="p-5 text-sm text-muted-foreground">No labels yet.</p>
        ) : (
          <ul className="divide-y divide-border">
            {contacts.data.map((c) => (
              <li key={c.address} className="flex items-center gap-4 px-5 py-2.5 text-sm">
                <span className="w-48 truncate font-medium">{c.label}</span>
                <CopyValue
                  value={c.address}
                  head={14}
                  tail={8}
                  label="address"
                  className="flex-1"
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label={`Remove ${c.label}`}
                  onClick={() => void remove(c.address)}
                >
                  <XIcon />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}
