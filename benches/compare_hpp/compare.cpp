#include <mapbox/earcut.hpp>
#include <array>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <cstdlib>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <string>
#include <set>
#include <vector>
#ifdef __APPLE__
#include <pthread/qos.h>
#endif

using Point = std::array<double, 2>;
using Polygon = std::vector<std::vector<Point>>;
struct Case { std::string name; std::vector<Polygon> polygons; };

void read(std::istream& in, char* out, std::size_t n) {
    if (!in.read(out, static_cast<std::streamsize>(n))) throw std::runtime_error("truncated benchmark input");
}
uint32_t read_u32(std::istream& in) {
    unsigned char b[4]; read(in, reinterpret_cast<char*>(b), 4);
    return uint32_t(b[0]) | (uint32_t(b[1]) << 8) | (uint32_t(b[2]) << 16) | (uint32_t(b[3]) << 24);
}
double read_f64(std::istream& in) {
    const uint64_t lo = read_u32(in), hi = read_u32(in);
    const uint64_t bits = lo | (hi << 32);
    double value; std::memcpy(&value, &bits, sizeof value); return value;
}
std::vector<Case> load(const char* path, const std::set<std::string>& filters) {
    std::ifstream in(path, std::ios::binary);
    char magic[8]; read(in, magic, 8);
    if (std::memcmp(magic, "ECBENCH1", 8)) throw std::runtime_error("invalid benchmark input");
    const auto n = read_u32(in);
    std::vector<Case> cases;
    for (uint32_t c = 0; c < n; ++c) {
        const auto len = read_u32(in); std::string name(len, '\0'); read(in, name.data(), len);
        Case data{std::move(name), {}};
        const bool selected = filters.empty() || filters.count(data.name);
        const auto polygons = read_u32(in);
        if (selected) data.polygons.reserve(polygons);
        for (uint32_t p = 0; p < polygons; ++p) {
            const auto vertices = read_u32(in), holes = read_u32(in);
            if (!selected) {
                in.seekg(std::streamoff(vertices) * 16 + std::streamoff(holes) * 4, std::ios::cur);
                if (!in) throw std::runtime_error("invalid skipped polygon");
                continue;
            }
            std::vector<Point> flat; flat.reserve(vertices);
            for (uint32_t v = 0; v < vertices; ++v) {
                const auto x = read_f64(in), y = read_f64(in); flat.push_back({x, y});
            }
            Polygon polygon; polygon.reserve(holes + 1);
            uint32_t first = 0;
            for (uint32_t h = 0; h <= holes; ++h) {
                const auto last = h == holes ? vertices : read_u32(in);
                if (last < first || last > vertices) throw std::runtime_error("invalid hole offset");
                polygon.emplace_back(flat.begin() + first, flat.begin() + last); first = last;
            }
            data.polygons.push_back(std::move(polygon));
        }
        if (selected) cases.push_back(std::move(data));
    }
    return cases;
}
void observe(const std::vector<uint32_t>& indices) {
    // Compiler barrier equivalent to consuming a Rust output slice with black_box.
    asm volatile("" : : "g"(indices.data()), "g"(indices.size()) : "memory");
}
template <typename F> double sample(F& run, std::chrono::milliseconds duration) {
    using Clock = std::chrono::steady_clock;
    auto start = Clock::now(); run();
    const unsigned batch = Clock::now() - start >= std::chrono::milliseconds(1) ? 1 : 64;
    start = Clock::now(); uint64_t calls = 0;
    do { for (unsigned i = 0; i < batch; ++i) run(); calls += batch; } while (Clock::now() - start < duration);
    return std::chrono::duration<double, std::nano>(Clock::now() - start).count() / double(calls);
}
int main(int argc, char** argv) {
    if (argc < 4) throw std::runtime_error("usage: compare INPUT SAMPLES SAMPLE_MS");
#ifdef __APPLE__
    if (pthread_set_qos_class_self_np(QOS_CLASS_USER_INITIATED, 0)) throw std::runtime_error("QoS setup failed");
#endif
    const std::set<std::string> filters(argv + 4, argv + argc);
    const auto cases = load(argv[1], filters);
    const auto rounds = std::stoul(argv[2]);
    const auto duration = std::chrono::milliseconds(std::stoul(argv[3]));
    std::cout << std::fixed << std::setprecision(6);
    const std::string mode = std::getenv("EARCUT_BENCH_MODE") ? std::getenv("EARCUT_BENCH_MODE") : "all";
    if (mode != "all" && mode != "cpp_once") throw std::runtime_error("invalid benchmark mode");
    for (const auto& data : cases) {
        auto once = [&] { for (const auto& polygon : data.polygons) { auto indices = mapbox::earcut<uint32_t>(polygon); observe(indices); } };
        sample(once, std::chrono::milliseconds(500));
        for (unsigned r = 0; r < rounds; ++r) {
            for (int i = 0; i < 3; ++i) once();
            const auto ns = sample(once, duration);
            std::cout << data.name << "\tcpp_once\t" << r << '\t' << ns << '\n';
        }
    }
}
