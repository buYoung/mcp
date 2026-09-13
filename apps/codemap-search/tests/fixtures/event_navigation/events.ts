import { KnownBus } from './known';
export const appBus = new KnownBus();
export const otherBus = new KnownBus();
export const SAVED = 'saved';
export function handleSaved() {}
appBus.on(SAVED, handleSaved);
