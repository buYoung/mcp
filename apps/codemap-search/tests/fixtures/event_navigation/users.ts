import { appBus as bus, SAVED } from './barrel';
export function save() { bus.emit(SAVED); }
