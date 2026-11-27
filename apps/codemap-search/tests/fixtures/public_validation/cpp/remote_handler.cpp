#include "contract.h"
#include "forward.h"

class RemoteHandler : public Handler {
 public:
  int priority() const override { return 2; }
  void reset() override {}
};
