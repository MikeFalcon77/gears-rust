# Gear UI packages

The community product composes source packages at build time with React 19,
TypeScript, Vite, Tailwind 4, TanStack Query, lucide and Firebase Web SDK.

- ui-sdk owns GearUiPlugin v1, runtime identity, authenticated API transport and
  identity/tenant query keys.
- product-shell owns authentication, the server-selected laboratory, cache and
  request lifetime, navigation, lazy pages/widgets, notifications and boundaries.
- Each gear owns its UI package. Settings lives in gears/settings-service/ui.

A package.json declares its npm name, version, exports and gearsUi metadata.
Use apiVersion 1 and kind sdk, shell or module. Modules export ./plugin from
src/ and expose a plugin object with pages, navigation and widgets. Shell and
modules declare @gears/ui-sdk as a ^1.0.0 peer. UI packages do not choose the
user's tenant: /family-labs/v1/me supplies it after authentication.

A gear's GDL points at its package with ui_module and lists its backend needs.
A product uses ui_product to select the shell, SDK and gear modules. Gearbox
validates the declarations and JSON metadata without executing JavaScript,
then copies source packages into a generated npm workspace. The companion
ui.lock.json records source hashes and the parent product lock hash; npm's
package-lock.json pins third-party dependencies for the prepared build.

Prepare public Firebase configuration with the community product's
prepare-ui.cjs script in Gearbox after generation, then run npm install once,
npm run build and npm test in the generated ui directory. Prepared Docker
builds use npm ci. The generated nginx serves the SPA and forwards /api to the
Kubernetes API Gateway with streaming buffering disabled.

Keep credentials, account mappings and family photographs out of these source
packages. Private presentation assets belong only in the ignored product bundle.
