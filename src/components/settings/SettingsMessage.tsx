import type { SettingsMessage } from "../../lib/settingsMessage";

export function SettingsMessage({ message }: { message: SettingsMessage }) {
  if (!message) return null;
  const isError = message.kind === "error";
  return (
    <p
      className={isError ? "settings-message settings-message-error" : "settings-message"}
      role={isError ? "alert" : "status"}
    >
      {message.text}
    </p>
  );
}
