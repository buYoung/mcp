#pragma once

#include <string>

class Named {
 public:
  virtual ~Named() = default;
  virtual void rename(const std::string& name) = 0;
};
