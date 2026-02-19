"use client";

import { use } from "react";
import { Table } from "@/components/Table";

export default function TablePage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = use(params);
  const tableId = parseInt(id, 10);

  if (isNaN(tableId) || tableId < 1) {
    return (
      <div className="min-h-screen flex items-center justify-center text-red-400">
        Invalid table ID
      </div>
    );
  }

  return <Table tableId={tableId} />;
}
