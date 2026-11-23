// The fake's side of 「新しい版をたしかめる」 (round 12, D1). The fake never looks on the net: scenes put a found
// version in `updates.newVersion` and the status line's `newVersion`.

import type { FakeState } from "./fake";
import type { Change, Command, StatusMessage } from "./types";

/** The switch, 「ページをひらく」 and 「あとで」 as Rust takes them. `null`: not one of these commands. */
export function reduceUpdates(state: FakeState, command: Command): Change[] | null {
  const updates = { ...state.updates };
  const said = state.status.message?.id === "newVersion";
  let message: StatusMessage | null | undefined = undefined;
  switch (command.type) {
    case "setCheckUpdates":
      if (updates.check === command.on) return [];
      updates.check = command.on;
      if (!command.on) {
        updates.newVersion = null;
        if (said) message = null;
      }
      break;
    case "openReleasePage":
    case "dismissNewVersion":
      if (updates.newVersion === null) return [];
      if (said) message = null;
      break;
    default:
      return null;
  }
  const changes: Change[] = [{ section: "updates", value: updates }];
  if (message !== undefined) changes.push({ section: "status", value: { message } });
  return changes;
}
