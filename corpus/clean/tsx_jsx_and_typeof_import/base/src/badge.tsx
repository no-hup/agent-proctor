import React from "react";
export function Badge({ label }: { label: string }) {
  return <span data-testid="badge">{label}</span>;
}
