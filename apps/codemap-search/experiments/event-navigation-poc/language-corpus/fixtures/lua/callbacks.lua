local Router = {}
function Router:setPrimary(cb) self.primary = cb end -- store_primary
function Router:setSecondary(cb) self.secondary = cb end -- store_secondary
function Router:firePrimary() self.primary() end -- call_primary
function Router:fireSecondary() self.secondary() end -- call_secondary
local Other = {}
function Other:firePrimary() self.primary() end -- call_other
return {Router = Router, Other = Other}
