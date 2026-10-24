import React, { useEffect, useState } from "react";
import {
  BookOpen,
  Calendar,
  ExternalLink,
  Filter,
  Globe,
  Newspaper,
  RefreshCw,
  Search,
  Tag,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { getArticle, getNews } from "../../lib/tauri/ipc";
import { NewsItemDto } from "../../types";

const CATEGORIES = ["ALL", "CRYPTO", "MACRO", "FED", "SEC"];

export const NewsWorkspace: React.FC = () => {
  const { activeAsset } = useWorkspaceStore();
  const [news, setNews] = useState<NewsItemDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedCategory, setSelectedCategory] = useState("ALL");
  const [search, setSearch] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [articleBody, setArticleBody] = useState<string | null>(null);
  const [loadingBody, setLoadingBody] = useState(false);

  // Fetch news list
  useEffect(() => {
    let isMounted = true;
    setLoading(true);

    getNews(selectedCategory === "ALL" ? undefined : selectedCategory)
      .then((items) => {
        if (isMounted) {
          setNews(items);
          setLoading(false);
          if (items.length > 0 && !selectedId) {
            setSelectedId(items[0].id);
          }
        }
      })
      .catch((err) => {
        console.error("Failed to load news:", err);
        if (isMounted) setLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, [selectedCategory]);

  const selectedItem = news.find((n) => n.id === selectedId) || news[0] || null;

  // Fetch article body when selectedItem changes
  useEffect(() => {
    if (!selectedItem) {
      setArticleBody(null);
      return;
    }

    if (selectedItem.body) {
      setArticleBody(selectedItem.body);
      return;
    }

    let isMounted = true;
    setLoadingBody(true);

    getArticle(selectedItem.id)
      .then((body) => {
        if (isMounted) {
          setArticleBody(body || selectedItem.summary);
          setLoadingBody(false);
        }
      })
      .catch((err) => {
        console.error("Failed to fetch article body:", err);
        if (isMounted) {
          setArticleBody(selectedItem.summary);
          setLoadingBody(false);
        }
      });

    return () => {
      isMounted = false;
    };
  }, [selectedItem]);

  const filteredNews = news.filter((item) => {
    if (!search.trim()) return true;
    const q = search.trim().toUpperCase();
    return (
      item.headline.toUpperCase().includes(q) ||
      (item.ticker?.toUpperCase().includes(q) ?? false) ||
      item.source.toUpperCase().includes(q)
    );
  });

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-hidden text-xs font-mono select-none">
      {/* Top News Filter Bar */}
      <div className="h-10 px-3 bg-surface border-b border-border flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <div className="flex items-center bg-surface-raised border border-border rounded p-0.5 space-x-0.5">
            {CATEGORIES.map((cat) => (
              <button
                key={cat}
                onClick={() => setSelectedCategory(cat)}
                className={`px-2 py-0.5 rounded text-2xs font-bold transition-colors ${
                  selectedCategory === cat
                    ? "bg-accent text-background"
                    : "text-foreground-muted hover:text-foreground"
                }`}
              >
                {cat}
              </button>
            ))}
          </div>
        </div>

        {/* Search Filter */}
        <div className="relative w-64">
          <Search className="w-3.5 h-3.5 text-foreground-muted absolute left-2 top-2" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search headlines or ticker..."
            className="w-full bg-surface-raised border border-border rounded pl-7 pr-2 py-0.5 text-2xs text-foreground placeholder:text-foreground-muted outline-none focus:border-accent"
          />
        </div>
      </div>

      {/* 3-Column Workstation Layout */}
      <div className="flex-1 flex overflow-hidden">
        {/* Column 1: Headline Feed */}
        <div className="w-80 md:w-96 border-r border-border flex flex-col bg-surface overflow-hidden">
          <div className="p-2 border-b border-border text-2xs font-bold text-foreground-muted flex justify-between items-center bg-surface-raised">
            <span>HEADLINES ({filteredNews.length})</span>
            {loading && <RefreshCw className="w-3 h-3 text-accent animate-spin" />}
          </div>

          <div className="flex-1 overflow-y-auto divide-y divide-border-subtle">
            {filteredNews.map((item) => {
              const isSelected = selectedItem?.id === item.id;
              const timeStr = new Date(item.published_at).toLocaleTimeString("en-US", {
                hour: "2-digit",
                minute: "2-digit",
                hour12: false,
              });

              return (
                <div
                  key={item.id}
                  onClick={() => setSelectedId(item.id)}
                  className={`p-2.5 cursor-pointer transition-colors ${
                    isSelected
                      ? "bg-accent/15 border-l-2 border-accent text-foreground"
                      : "hover:bg-surface-raised text-foreground-muted hover:text-foreground"
                  }`}
                >
                  <div className="flex items-center justify-between text-2xs mb-1">
                    <span className="text-accent font-bold">{item.source}</span>
                    <span className="text-foreground-muted">{timeStr}</span>
                  </div>
                  <h4 className="font-sans font-medium text-xs line-clamp-2 text-foreground mb-1 leading-snug">
                    {item.headline}
                  </h4>
                  <div className="flex items-center space-x-2 text-2xs">
                    {item.ticker && (
                      <span className="px-1.5 py-0.2 rounded bg-surface-hover border border-border text-accent font-bold">
                        {item.ticker}
                      </span>
                    )}
                    <span className="text-foreground-muted uppercase">
                      {item.category}
                    </span>
                    {item.is_sec_filing && (
                      <span className="text-terminal-cyan font-bold">
                        SEC {item.form}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Column 2: Article Reader */}
        <div className="flex-1 flex flex-col bg-background overflow-hidden border-r border-border">
          {selectedItem ? (
            <div className="flex-1 flex flex-col overflow-y-auto p-6 max-w-3xl mx-auto w-full">
              {/* Article Header */}
              <div className="border-b border-border pb-4 mb-4">
                <div className="flex items-center space-x-2 text-2xs text-accent font-bold mb-2">
                  <span>{selectedItem.source.toUpperCase()}</span>
                  <span>·</span>
                  <span>{selectedItem.category.toUpperCase()}</span>
                  {selectedItem.ticker && (
                    <>
                      <span>·</span>
                      <span className="bg-accent/20 px-1.5 py-0.5 rounded text-accent">
                        ${selectedItem.ticker}
                      </span>
                    </>
                  )}
                </div>

                <h1 className="font-sans font-bold text-lg md:text-xl text-foreground leading-tight mb-2">
                  {selectedItem.headline}
                </h1>

                <div className="flex items-center justify-between text-2xs text-foreground-muted">
                  <span>Published {new Date(selectedItem.published_at).toLocaleString()}</span>
                  <a
                    href={selectedItem.url}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center space-x-1 text-accent hover:underline"
                  >
                    <span>Original Source</span>
                    <ExternalLink className="w-3 h-3" />
                  </a>
                </div>
              </div>

              {/* Clean Article Body */}
              <div className="font-sans text-sm text-foreground/90 leading-relaxed space-y-4">
                {loadingBody ? (
                  <div className="flex items-center space-x-2 text-foreground-muted py-8">
                    <RefreshCw className="w-4 h-4 text-accent animate-spin" />
                    <span>Extracting clean article text...</span>
                  </div>
                ) : (
                  (articleBody || selectedItem.summary)
                    .split("\n\n")
                    .map((paragraph, idx) => (
                      <p key={idx} className="text-xs md:text-sm">
                        {paragraph}
                      </p>
                    ))
                )}
              </div>
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center text-foreground-muted">
              Select an article to read.
            </div>
          )}
        </div>

        {/* Column 3: Context & Metadata Inspector */}
        <div className="w-72 bg-surface p-3 flex flex-col space-y-3 overflow-y-auto hidden xl:flex">
          <div className="text-2xs font-bold text-foreground-muted uppercase tracking-wider">
            Context & Provenance
          </div>

          {selectedItem && (
            <>
              <div className="bg-surface-raised border border-border rounded p-2.5 space-y-2 text-2xs">
                <div>
                  <span className="text-foreground-muted block">Publisher:</span>
                  <span className="text-foreground font-bold">{selectedItem.source}</span>
                </div>
                <div>
                  <span className="text-foreground-muted block">Classification:</span>
                  <span className="text-accent">{selectedItem.category}</span>
                </div>
                <div>
                  <span className="text-foreground-muted block">Related Instrument:</span>
                  <span className="text-foreground font-bold">
                    {selectedItem.ticker || "Macro / Market-wide"}
                  </span>
                </div>
                {selectedItem.is_sec_filing && (
                  <div>
                    <span className="text-foreground-muted block">Filing Form:</span>
                    <span className="text-terminal-cyan font-bold">
                      SEC Form {selectedItem.form}
                    </span>
                  </div>
                )}
              </div>

              <div className="bg-surface-raised border border-border rounded p-2.5 space-y-2 text-2xs">
                <span className="text-foreground-muted font-bold block uppercase">
                  Extraction Pipeline
                </span>
                <p className="text-foreground-muted">
                  Content sanitized in Rust. Tracking scripts, cookie banners, navigation boilerplate, and ads stripped.
                </p>
                <a
                  href={selectedItem.url}
                  target="_blank"
                  rel="noreferrer"
                  className="w-full inline-flex items-center justify-center space-x-1.5 px-3 py-1.5 bg-surface hover:bg-surface-hover border border-border rounded text-accent font-bold transition-colors"
                >
                  <span>Open Publisher Link</span>
                  <ExternalLink className="w-3.5 h-3.5" />
                </a>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
};
