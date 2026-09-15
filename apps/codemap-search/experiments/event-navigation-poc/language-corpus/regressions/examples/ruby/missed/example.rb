class Router
  def install(f)
    @cb = f # S
  end
  def fire
    instance_variable_get(:@cb).call # I
  end
end
