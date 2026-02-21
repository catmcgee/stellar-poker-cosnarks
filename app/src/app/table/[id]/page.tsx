"use client";

import { use } from "react";
import { useSearchParams } from "next/navigation";
import { Table } from "@/components/Table";

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
    mode === "single" ? "single" : undefined;

  if (isNaN(tableId) || tableId < 0) {
    return (
      <div className="min-h-screen flex items-center justify-center text-red-400">
        Invalid table ID
      </div>
    );
  }

  return <Table tableId={tableId} initialPlayMode={initialPlayMode} />;
}
