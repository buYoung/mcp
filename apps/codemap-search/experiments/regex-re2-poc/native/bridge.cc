#include <cstdio>
#include <limits>
#include <vector>
#include <re2/re2.h>

// The bridge borrows the original Rust buffer. It does not copy strings or
// consume prefixes: Match(startpos) must retain anchor/boundary context.
struct Span { size_t start; size_t end; };
constexpr size_t kMaxCaptures = 128;
struct Compiled {
    RE2 regex;
    mutable std::vector<absl::string_view> groups;
    Compiled(absl::string_view pattern, const RE2::Options& options)
        : regex(pattern, options), groups(regex.ok() ? regex.NumberOfCapturingGroups() + 1 : 1) {}
};

extern "C" {
void* poc_re2_new(const char* pattern, size_t length, char* error, size_t capacity) {
    RE2::Options options;
    options.set_log_errors(false);
    auto* compiled = new Compiled(absl::string_view(pattern, length), options);
    if (!compiled->regex.ok() || compiled->groups.size() > kMaxCaptures) {
        const auto message = compiled->regex.ok() ? "PoC capture limit (128) exceeded" : compiled->regex.error();
        std::snprintf(error, capacity, "%s", message.c_str());
        delete compiled;
        return nullptr;
    }
    return compiled;
}

void poc_re2_free(void* regex) { delete static_cast<Compiled*>(regex); }

size_t poc_re2_capture_count(const void* regex) {
    return static_cast<const Compiled*>(regex)->groups.size();
}

bool poc_re2_match(const void* regex, const char* data, size_t length,
                   size_t start, Span* spans, size_t count) {
    if (count > kMaxCaptures || start > length) return false;
    const auto* compiled = static_cast<const Compiled*>(regex);
    auto& groups = compiled->groups;
    if (count > groups.size()) return false;
    if (!compiled->regex.Match(absl::string_view(data, length),
            start, length, RE2::UNANCHORED, count ? groups.data() : nullptr, static_cast<int>(count))) {
        return false;
    }
    for (size_t i = 0; i < count; ++i) {
        if (groups[i].data() == nullptr) {
            spans[i] = {std::numeric_limits<size_t>::max(), std::numeric_limits<size_t>::max()};
        } else {
            auto offset = static_cast<size_t>(groups[i].data() - data);
            spans[i] = {offset, offset + groups[i].size()};
        }
    }
    return true;
}
}
