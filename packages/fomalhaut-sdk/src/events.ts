import type { Event, EventEnvelope } from "./generated/v1/index.js";

export type FomalhautEventName = Event["event"];
export type FomalhautEventData<Name extends FomalhautEventName> = Extract<
  Event,
  { event: Name }
>["data"];
export type FomalhautEventListener<Name extends FomalhautEventName> = (
  data: FomalhautEventData<Name>,
  envelope: EventEnvelope,
) => void;
