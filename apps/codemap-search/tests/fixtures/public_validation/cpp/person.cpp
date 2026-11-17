#include "named.h"

class Person : public Named {
 public:
  void rename(const std::string& name) override {}
};
