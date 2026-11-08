import { Component, type ErrorInfo, type ReactNode } from "react";
import { withTranslation, type WithTranslation } from "react-i18next";
import { logUi } from "../lib/api";
import { Button } from "./ui";

interface State {
  failed: boolean;
}

class Boundary extends Component<WithTranslation & { children: ReactNode }, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    const stack = info.componentStack?.split("\n").slice(0, 4).join(" | ") ?? "";
    logUi("error", `render crash: ${error.message} ${stack}`);
  }

  render() {
    const { t, children } = this.props;
    if (!this.state.failed) return children;
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 bg-app px-8 text-center">
        <h1 className="text-xl font-semibold">{t("crash.title")}</h1>
        <p className="max-w-[46ch] text-sm text-muted">{t("crash.body")}</p>
        <Button variant="primary" onClick={() => window.location.reload()}>
          {t("crash.reload")}
        </Button>
      </div>
    );
  }
}

export const ErrorBoundary = withTranslation()(Boundary);
