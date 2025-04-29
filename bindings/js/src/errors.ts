import type {
  FailureArtifactRef,
  FailureDetails,
} from "./types.js";

export type ErrorKind = "assertion" | "usage" | "no_session" | "internal";

export class TermprobeError extends Error {
  readonly kind: ErrorKind;
  readonly exitCode: number;
  readonly details?: FailureDetails;
  readonly artifact?: FailureArtifactRef;

  constructor(message: string, kind: ErrorKind = "internal", exitCode = 5) {
    super(message);
    this.name = new.target.name;
    this.kind = kind;
    this.exitCode = exitCode;
  }
}

export class ExpectationError extends TermprobeError {
  constructor(message: string) {
    super(message, "assertion", 1);
  }
}

export class UsageError extends TermprobeError {
  constructor(message: string) {
    super(message, "usage", 2);
  }
}

export class NoSessionError extends TermprobeError {
  constructor(message: string) {
    super(message, "no_session", 3);
  }
}

export class InternalError extends TermprobeError {
  constructor(message: string) {
    super(message, "internal", 5);
  }
}
