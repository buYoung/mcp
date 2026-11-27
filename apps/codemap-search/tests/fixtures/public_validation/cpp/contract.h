#ifndef CODEMAP_REGRESSION_CONTRACT_H_
#define CODEMAP_REGRESSION_CONTRACT_H_

class Handler {
 public:
  Handler();
  virtual ~Handler();
  virtual int priority() const = 0;
  virtual void reset() = 0;
};

class LocalHandler : public Handler {
 public:
  int priority() const override;
  void reset() override;
};

#ifdef CODEMAP_REGRESSION_EXTRA
class ExtraHandler : public Handler {
 public:
  int priority() const override;
  void reset() override;
};
#endif

#endif  // CODEMAP_REGRESSION_CONTRACT_H_
