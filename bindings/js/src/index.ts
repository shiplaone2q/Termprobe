export { Termprobe } from "./client.js";
export type {
  ClipboardWaitOptions,
  LocatorClickOptions,
  LocatorDirection,
  LocatorExpectOptions,
  LocatorHighlightOptions,
  LocatorFilterOptions,
  LinkSelectorOptions,
  RelativeLinkSelectorOptions,
  LocatorWaitOptions,
  Locator,
  RelativeStyleSelectorOptions,
  RelativeTextSelectorOptions,
  StyleSelectorOptions,
  TextSelectorOptions,
  TextStyleExpectation,
  MouseButton,
  MouseButtonOptions,
  MouseClickOptions,
  TitleOptions,
  RecordingFormat,
  RecordingOptions,
  RestartOptions,
  ScreenshotOptions,
} from "./client.js";
export { uniqueSession } from "./ephemeral.js";
export { closeAll, getRecording, sessions } from "./sessions.js";
export {
  ExpectationError,
  InternalError,
  NoSessionError,
  TermprobeError,
  UsageError,
} from "./errors.js";
export type { ErrorKind } from "./errors.js";
export { VERSION } from "./version.js";
export type {
  ArtifactOptions,
  AutomaticRecording,
  TraceOptions,
  Backend,
  BellEvent,
  Cell,
