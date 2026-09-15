class Router
  def install(f, g)
    @cb = f # @S1
    @other = g # @S2
  end
  def fire
    instance_variable_get(:@cb).call # @I1
  end
end
