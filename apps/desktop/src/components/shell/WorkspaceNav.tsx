import React from "react";
import {
  BarChart3,
  BookOpen,
  Briefcase,
  CandlestickChart,
  Globe,
  LayoutGrid,
  LineChart,
  Newspaper,
  SlidersHorizontal,
  Table,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { WorkspaceType } from "../../types";

interface NavItem {
  id: WorkspaceType;
  label: string;
  mnemonic: string;
  icon: React.ReactNode;
}

const NAV_ITEMS: NavItem[] = [
  {
    id: "LAUNCHPAD",
    label: "Launchpad",
    mnemonic: "LP",
    icon: <LayoutGrid className="w-3.5 h-3.5" />,
  },
  {
    id: "MARKET",
    label: "Markets",
    mnemonic: "MKT",
    icon: <Table className="w-3.5 h-3.5" />,
  },
  {
    id: "CHART",
    label: "Chart",
    mnemonic: "GP",
    icon: <CandlestickChart className="w-3.5 h-3.5" />,
  },
  {
    id: "FIN",
    label: "Financials",
    mnemonic: "FA",
    icon: <BarChart3 className="w-3.5 h-3.5" />,
  },
  {
    id: "NEWS",
    label: "News",
    mnemonic: "CN",
    icon: <Newspaper className="w-3.5 h-3.5" />,
  },
  {
    id: "SCAN",
    label: "Screener",
    mnemonic: "EQS",
    icon: <SlidersHorizontal className="w-3.5 h-3.5" />,
  },
  {
    id: "PORT",
    label: "Portfolio",
    mnemonic: "PORT",
    icon: <Briefcase className="w-3.5 h-3.5" />,
  },
  {
    id: "ECON",
    label: "Economics",
    mnemonic: "ECON",
    icon: <Globe className="w-3.5 h-3.5" />,
  },
  {
    id: "DES",
    label: "Description",
    mnemonic: "DES",
    icon: <BookOpen className="w-3.5 h-3.5" />,
  },
];

export const WorkspaceNav: React.FC = () => {
  const { workspace, setWorkspace } = useWorkspaceStore();

  return (
    <nav className="h-8 bg-background border-b border-border flex items-center justify-between px-3 text-xs select-none">
      <div className="flex items-center space-x-1 overflow-x-auto">
        {NAV_ITEMS.map((item) => {
          const isActive = workspace === item.id;
          return (
            <button
              key={item.id}
              onClick={() => setWorkspace(item.id)}
              className={`flex items-center space-x-1.5 px-2.5 py-1 rounded text-2xs font-mono transition-colors border ${
                isActive
                  ? "bg-accent text-background border-accent font-bold shadow-sm"
                  : "bg-surface border-border text-foreground-muted hover:text-foreground hover:bg-surface-raised"
              }`}
            >
              <span className={isActive ? "text-background" : "text-accent"}>
                {item.icon}
              </span>
              <span>{item.label}</span>
              <span
                className={`text-2xs px-1 rounded ${
                  isActive ? "bg-background/20 text-background" : "bg-background text-foreground-muted"
                }`}
              >
                {item.mnemonic}
              </span>
            </button>
          );
        })}
      </div>

      <div className="flex items-center space-x-2 text-2xs text-foreground-muted font-mono hidden md:flex">
        <span>[ / ] Search</span>
        <span>[ : ] Go</span>
        <span>[ Tab ] Cycle Panes</span>
      </div>
    </nav>
  );
};
