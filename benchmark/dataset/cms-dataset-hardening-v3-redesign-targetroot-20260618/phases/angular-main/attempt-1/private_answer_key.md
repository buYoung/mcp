# Private Answer Key

## Candidate

- id: `angular-hidden-shell-lifetime-v1`
- status: pending validation

## Correct Answer

The decisive rule is not simple "visible tree" activity. Angular's later cleanup logic reconstructs activity from both the current router state and every stored detached handle. If a preserved deep child is still sitting in the reuse store, Angular walks that stored child's live route snapshot ancestry and marks each ancestor route config as active. That is why an invisible workspace shell can still keep its route-scoped resources alive.

This only happens because the detach/store path preserves live route state instead of tearing it down:

1. Departure first forks on `shouldDetach(...)`. If the route has a component and the reuse strategy says to detach, Angular takes `detachAndStoreRouteSubtree(...)` instead of `deactivateRouteAndOutlet(...)`. The normal deactivate path is the one that destroys `route.value._localInjector`; the detach/store path skips that destruction. Evidence: `packages/router/src/operators/activate_routes.ts:89-93`, `:96-112`, `:142-147`.
2. The stored handle retains live state, not just metadata. Angular stores `{componentRef, route, contexts}`, where `route` is the live `TreeNode<ActivatedRoute>`. That retained route still carries the live `ActivatedRoute`, including `_localInjector`, and exposes `snapshot.pathFromRoot`. Evidence: `packages/router/src/operators/activate_routes.ts:108-111`, `packages/router/src/route_reuse_strategy.ts:23-29`, `:42-52`.
3. Later cleanup is a separate mechanism that runs on `NavigationEnd` only when the experimental injector auto-cleanup feature is provided. The router calls `injectorCleanup?.(this.routeReuseStrategy, this.routerState, this.config)` from the `NavigationEnd` branch. Evidence: `packages/router/src/router.ts:208-212`, `packages/router/src/provide_router.ts:746-763`.
4. That cleanup first collects active routes from the current router state, then also collects every ancestor of every stored handle by iterating `internalHandle.route.value.snapshot.pathFromRoot`. Each ancestor `snapshot.routeConfig` is added into `activeRoutes`. This is the hidden rule that keeps the invisible shell treated as active. Evidence: `packages/router/src/route_injector_cleanup.ts:26-46`.
5. Route-config injector destruction is additionally gated. A route's `_injector` or `_loadedInjector` is destroyed only when the route is inactive, has an injector, and `shouldDestroyInjector(route)` returns true; descendant teardown can cascade through `inheritedForceDestroy`. Evidence: `packages/router/src/route_injector_cleanup.ts:58-96`.
6. Final cleanup therefore needs both state removal and another cleanup opportunity. Once the preserved handle is dropped, a later `NavigationEnd` can remove the ancestor from `activeRoutes`; alternatively, `destroyDetachedRouteHandle(...)` explicitly destroys the stored component and retained `_localInjector`. Evidence: `packages/router/src/route_reuse_strategy.ts:42-52`, `packages/router/test/route_injector_cleanup.spec.ts:181-191`, `:230-253`, `:301-320`.

## Pinned Facts

- The hidden activity rule comes from `pathFromRoot` ancestry of stored detached handles, not only from the currently visible router tree.
- The preserved state must include the live detached `route` tree inside the stored handle, not just serialized route metadata.
- The detach/store fork is controlled by `shouldDetach(...)`, and that fork preserves `_localInjector` by skipping the normal deactivate path.
- Later cleanup of route-config injectors is opt-in and runs from the router's `NavigationEnd` branch.
- Route-config injector destruction also requires inactivity plus `shouldDestroyInjector(route)`, with descendant force-destroy propagation from a destroyed parent.
- Full release happens only after the stored handle stops covering that ancestry and cleanup runs again, or when `destroyDetachedRouteHandle(...)` is called for the detached subtree.

## Insufficient Answers

- Answers that say only "the current active router tree keeps the shell alive."
- Answers that mention route reuse or caching but omit stored-handle ancestry via `pathFromRoot`.
- Answers that mention only `_localInjector` retention and not the separate route-config injector cleanup rule.
- Answers that omit the opt-in cleanup feature or `shouldDestroyInjector(route)` gate while claiming to explain the exact rule.
