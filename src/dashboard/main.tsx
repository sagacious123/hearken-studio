import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { blockContextMenu } from "../lib/contextMenu";
import { Dashboard } from "./Dashboard";
import "./styles.css";

blockContextMenu();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Dashboard />
  </StrictMode>,
);
