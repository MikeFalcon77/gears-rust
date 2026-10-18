import {
  Component,
  Suspense,
  lazy,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { initializeApp } from "firebase/app";
import {
  browserSessionPersistence,
  getAuth,
  onAuthStateChanged,
  setPersistence,
  signInWithPopup,
  GoogleAuthProvider,
  signOut,
  type User,
} from "firebase/auth";
import {
  Activity,
  Boxes,
  LogOut,
  Settings2,
  ShieldCheck,
  ArrowUpRight,
} from "lucide-react";
import {
  ApiClient,
  EventFeed,
  browserTabId,
  type GearUiPlugin,
  type GearRuntime,
  type Identity,
  type RuntimeConfig,
} from "@gears/ui-sdk";
class PageBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? (
      <div role="alert" className="empty-state">
        This module could not be loaded. Reload the page to retry.
      </div>
    ) : (
      this.props.children
    );
  }
}
export function ProductShell(props: {
  config: RuntimeConfig;
  modules: ReadonlyArray<GearUiPlugin>;
}) {
  const { config, modules } = props;
  const ids = modules.flatMap((module) => module.pages.map((page) => page.id));
  if (
    !config.firebase?.apiKey ||
    !config.firebase?.projectId ||
    !config.firebase?.authDomain
  )
    return (
      <main className="sign-in">
        <h1>Product configuration unavailable</h1>
        <p>
          The Firebase Web SDK configuration must be prepared before deployment.
        </p>
      </main>
    );
  if (
    modules.some((module) => module.apiVersion !== 1) ||
    new Set(ids).size !== ids.length
  )
    return (
      <main className="sign-in">
        <h1>Incompatible UI module</h1>
      </main>
    );
  return <ConfiguredShell {...props} />;
}
function ConfiguredShell({
  config,
  modules,
}: {
  config: RuntimeConfig;
  modules: ReadonlyArray<GearUiPlugin>;
}) {
  const [user, setUser] = useState<User | null>(null),
    [runtime, setRuntime] = useState<GearRuntime | null>(null),
    [status, setStatus] = useState("Checking your session…"),
    [error, setError] = useState(""),
    [toast, setToast] = useState("");
  const [pageId, setPageId] = useState(() => location.hash.slice(1));
  const generation = useRef(0);
  const auth = useMemo(() => getAuth(initializeApp(config.firebase)), [config]);
  const query = useMemo(
    () =>
      new QueryClient({
        defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: false } },
      }),
    [],
  );
  const pages = useMemo(
    () =>
      modules.flatMap((m) =>
        m.pages.map((p) => ({ ...p, View: lazy(p.load), module: m.id })),
      ),
    [modules],
  );
  const widgets = useMemo(
    () =>
      modules.flatMap((m) =>
        m.widgets.map((w) => ({ ...w, View: lazy(w.load) })),
      ),
    [modules],
  );
  const page = pages.find((p) => p.id === pageId) || pages[0];
  useEffect(() => {
    const change = () => setPageId(location.hash.slice(1));
    addEventListener("hashchange", change);
    return () => removeEventListener("hashchange", change);
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(""), 5000);
    return () => clearTimeout(timer);
  }, [toast]);
  useEffect(() => {
    let unsubscribe = () => {},
      closed = false,
      session: AbortController | undefined;
    setPersistence(auth, browserSessionPersistence)
      .then(() => {
        if (closed) return;
        unsubscribe = onAuthStateChanged(auth, async (next) => {
          const current = ++generation.current;
          session?.abort();
          session = new AbortController();
          setRuntime(null);
          setUser(next);
          setError("");
          await query.cancelQueries();
          query.clear();
          if (!next) {
            setStatus("Sign in to your laboratory.");
            return;
          }
          const abort = session;
          setStatus("Verifying your laboratory…");
          try {
            const api = new ApiClient(
              config.apiBase,
              (force) => next.getIdToken(force),
              abort.signal,
            );
            const identity = await api.json<Identity>(config.identityPath);
            if (!identity.subject_id || !identity.tenant_id)
              throw Error("Invalid identity response");
            if (closed || generation.current !== current) return;
            setRuntime({
              events: new EventFeed({
                api,
                identity,
                signal: abort.signal,
                tabId: browserTabId(),
                storage: sessionStorage,
                topic: config.events.topic,
                eventType: config.events.type,
              }),
              api,
              identity,
              query,
              config,
              signal: abort.signal,
              notify: setToast,
            });
            setStatus("");
          } catch (cause) {
            if (!abort.signal.aborted && !closed) {
              setError(
                cause instanceof Error
                  ? cause.message
                  : "Sign-in could not be verified.",
              );
              setStatus("Access could not be verified.");
            }
          }
        });
      })
      .catch(() => {
        if (!closed) setError("Authentication configuration is unavailable.");
      });
    return () => {
      closed = true;
      generation.current++;
      unsubscribe();
      session?.abort();
      void query.cancelQueries();
      query.clear();
    };
  }, [auth, query, config]);
  const login = async () => {
    setError("");
    try {
      const provider = new GoogleAuthProvider();
      provider.setCustomParameters({ prompt: "select_account" });
      await signInWithPopup(auth, provider);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Sign-in failed.");
    }
  };
  const logout = async () => {
    setRuntime(null);
    await signOut(auth);
  };
  if (modules.some((m) => m.apiVersion !== 1))
    return (
      <main className="sign-in">
        <h1>Incompatible UI module</h1>
      </main>
    );
  if (!runtime)
    return (
      <main className="sign-in">
        <div className="brand-mark">
          <Boxes size={30} />
        </div>
        <p className="eyebrow">GEARBOX COMMUNITY DEMO</p>
        <h1>{config.productName}</h1>
        <p>{status}</p>
        {error && (
          <p role="alert" className="error">
            {error}
          </p>
        )}
        <button onClick={() => void (user ? logout() : login())}>
          {user ? "Sign out and try another account" : "Sign in with Google"}
          <ArrowUpRight size={17} />
        </button>
        <span className="security-note">
          <ShieldCheck size={15} /> Your laboratory is selected by the server.
        </span>
      </main>
    );
  const name =
    config.tenants[runtime.identity.tenant_id]?.name || "Your laboratory";
  return (
    <QueryClientProvider client={query}>
      <div className="product-shell">
        <aside>
          <a className="wordmark" href="#">
            <Boxes size={27} />
            <span>
              {config.productName}
              <small>POWERED BY GEARS</small>
            </span>
          </a>
          <div className="tenant-label">
            <span className="status-dot" />
            {name}
          </div>
          <nav aria-label="Product modules">
            {modules
              .flatMap((m) => m.navigation)
              .map((n) => (
                <a
                  key={n.pageId}
                  href={"#" + n.pageId}
                  aria-current={n.pageId === page?.id ? "page" : undefined}
                >
                  {n.icon === "activity" ? (
                    <Activity size={18} />
                  ) : (
                    <Settings2 size={18} />
                  )}
                  <span>{n.label}</span>
                </a>
              ))}
          </nav>
          <div className="sidebar-footer">
            <ShieldCheck size={16} />
            <span>Tenant isolation enabled</span>
          </div>
        </aside>
        <section className="workspace">
          <header>
            <div>
              <span className="eyebrow">YOUR WORKSPACE</span>
              <strong>{name}</strong>
            </div>
            <div className="account">
              <span>{user?.displayName || "Google account"}</span>
              <button
                className="icon-button"
                aria-label="Sign out"
                onClick={() => void logout()}
              >
                <LogOut size={17} />
              </button>
            </div>
          </header>
          <main>
            <div className="page-heading">
              <div>
                <p className="eyebrow">
                  {config.productName.toUpperCase()} / {page?.module}
                </p>
                <h1>{page?.title || "Welcome"}</h1>
              </div>
              <span className="version-badge">Gears · Live product</span>
            </div>
            <div className={widgets.length ? "content-layout" : ""}>
              <section>
                <PageBoundary
                  key={page?.id + "-" + runtime.identity.subject_id}
                >
                  <Suspense
                    fallback={
                      <div className="empty-state" role="status">
                        Loading module…
                      </div>
                    }
                  >
                    {page && <page.View runtime={runtime} />}
                  </Suspense>
                </PageBoundary>
              </section>
              {widgets.length > 0 && (
                <section
                  className="widget-rail"
                  aria-label="Laboratory widgets"
                >
                  {widgets.map((widget) => (
                    <PageBoundary
                      key={widget.id + "-" + runtime.identity.subject_id}
                    >
                      <Suspense
                        fallback={
                          <div className="panel" role="status">
                            Loading laboratory widget…
                          </div>
                        }
                      >
                        <widget.View runtime={runtime} />
                      </Suspense>
                    </PageBoundary>
                  ))}
                </section>
              )}
            </div>
          </main>
          <footer>
            Declared in GDL. Running on Kubernetes.
            <span>{config.productName} · Community Edition</span>
          </footer>
        </section>
        {toast && (
          <div className="toast" role="status">
            {toast}
          </div>
        )}
      </div>
    </QueryClientProvider>
  );
}
