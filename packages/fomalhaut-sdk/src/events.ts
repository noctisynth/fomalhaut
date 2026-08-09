import type {
  Event,
  EventEnvelope,
  RuntimeMode,
} from "./generated/v1/index.js";

type RoleEventPattern<M extends RuntimeMode> = M extends "greeter"
  ? `session.${string}`
  : `lock.${string}`;

type CommonEventName = Exclude<
  Event["event"],
  `session.${string}` | `lock.${string}`
>;

/** Event names available to one host mode. */
export type FomalhautEventName<M extends RuntimeMode = RuntimeMode> =
  | CommonEventName
  | Extract<Event["event"], RoleEventPattern<M>>;

/** Generated wire event narrowed to one host mode. */
export type FomalhautEvent<M extends RuntimeMode> = Extract<
  Event,
  { event: FomalhautEventName<M> }
>;

/** Generated event envelope narrowed to one host mode and event name. */
export type FomalhautEventEnvelope<
  M extends RuntimeMode,
  Name extends FomalhautEventName<M> = FomalhautEventName<M>,
> = Extract<EventEnvelope, { event: Name }>;

export type FomalhautEventData<
  M extends RuntimeMode,
  Name extends FomalhautEventName<M>,
> = Extract<FomalhautEvent<M>, { event: Name }>["data"];

export type FomalhautEventListener<
  M extends RuntimeMode,
  Name extends FomalhautEventName<M>,
> = (
  data: FomalhautEventData<M, Name>,
  envelope: FomalhautEventEnvelope<M, Name>,
) => void;
