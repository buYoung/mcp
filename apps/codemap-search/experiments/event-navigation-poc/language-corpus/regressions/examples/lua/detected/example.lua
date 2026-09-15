local Router = {}
function Router:install(f) self.cb = f end -- S
function Router:fire() self.cb() end -- I
return Router
