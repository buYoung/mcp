local Router = {}
function Router:install(f, g)
    self.cb = f -- @S1
    self.other = g -- @S2
    self.lookup = {}
    self.lookup.cb = f -- @S3
end
function Router:fire() self["cb"]() end -- @I1
function Router:map() self.lookup["cb"]() end -- @I2
return Router
