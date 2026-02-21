"use client";

import { use } from "react";
import { useSearchParams } from "next/navigation";
import { Table } from "@/components/Table";
import { PixelWorld } from "@/components/PixelWorld";

export default function TablePage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = use(params);
  const searchParams = useSearchParams();
  const tableId = parseInt(id, 10);
  const mode = searchParams.get("mode");
  const initialPlayMode =
    mode === "single" || mode === "headsup" || mode === "multi"
      ? mode
      : undefined;

  if (isNaN(tableId) || tableId < 0) {
    return (
      <PixelWorld>
        <div className="min-h-screen flex items-center justify-center p-6">
          <div
            className="pixel-border-thin px-4 py-3 text-[10px]"
            style={{
              background: "rgba(12, 10, 24, 0.88)",
              color: "#ff7675",
              borderColor: "#c47d2e",
            }}
          >
            INVALID TABLE ID
          </div>
        </div>
      </PixelWorld>
    );
  }

  return <Table tableId={tableId} initialPlayMode={initialPlayMode} />;
}
