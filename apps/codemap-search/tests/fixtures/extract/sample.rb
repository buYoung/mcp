require_relative "worker"

module Demo
  class Worker
    attr_reader :status, :name
    STATE = "ready"

    def run(clock)
      helper()
      clock.tick()
    end

    private

    def helper
    end
  end
end

def top_level
  Demo::Worker.new
end

