class Router
  def install(f)
    @cb = f # S
  end
  def fire
    @cb.call # I
  end
end
