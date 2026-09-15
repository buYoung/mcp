class Router
  def set_primary(cb)
    @primary = cb # store_primary
  end
  def set_secondary(cb)
    @secondary = cb # store_secondary
  end
  def fire_primary
    @primary.call # call_primary
  end
  def fire_secondary
    @secondary.call # call_secondary
  end
end
class Other
  def fire_primary
    @primary.call # call_other
  end
end
