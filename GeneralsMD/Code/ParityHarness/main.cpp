// Portable C++ producer for the shared frame-trace scenario.
//
// This is intentionally a component harness, not a pretend port of the
// engine. It executes the original GeneralsMD RandomValue.cpp through
// original_random_adapter.cpp. The fixture supplies the observable object,
// player, and command records because the full Windows engine cannot be
// linked on this host. The output labels that authority boundary explicitly.

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <map>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <string>
#include <utility>
#include <variant>
#include <vector>

extern "C" const std::uint32_t *generalsmd_logic_seed(void);
extern "C" void generalsmd_init_logic_random(std::uint32_t seed);
extern "C" void generalsmd_draw_logic_random(void);

namespace {

struct Json {
    enum class Kind { null_value, boolean, number, string, array, object };

    Kind kind = Kind::null_value;
    bool boolean = false;
    double number = 0.0;
    std::string string;
    std::vector<Json> array;
    std::map<std::string, Json> object;

    static Json null() { return {}; }
    static Json boolean_value(bool value) {
        Json json;
        json.kind = Kind::boolean;
        json.boolean = value;
        return json;
    }
    static Json number_value(double value) {
        Json json;
        json.kind = Kind::number;
        json.number = value;
        return json;
    }
    static Json string_value(std::string value) {
        Json json;
        json.kind = Kind::string;
        json.string = std::move(value);
        return json;
    }
    static Json array_value(std::vector<Json> value) {
        Json json;
        json.kind = Kind::array;
        json.array = std::move(value);
        return json;
    }
    static Json object_value(std::map<std::string, Json> value) {
        Json json;
        json.kind = Kind::object;
        json.object = std::move(value);
        return json;
    }
};

class JsonParser {
public:
    explicit JsonParser(const std::string &input) : input_(input) {}

    Json parse() {
        Json value = parse_value();
        skip_space();
        if (position_ != input_.size()) {
            fail("trailing data");
        }
        return value;
    }

private:
    const std::string &input_;
    std::size_t position_ = 0;

    [[noreturn]] void fail(const std::string &message) const {
        throw std::runtime_error("scenario JSON at byte " + std::to_string(position_) + ": " + message);
    }

    void skip_space() {
        while (position_ < input_.size()) {
            const char character = input_[position_];
            if (character != ' ' && character != '\n' && character != '\r' && character != '\t') {
                return;
            }
            ++position_;
        }
    }

    bool consume(char expected) {
        skip_space();
        if (position_ < input_.size() && input_[position_] == expected) {
            ++position_;
            return true;
        }
        return false;
    }

    void expect(char expected) {
        if (!consume(expected)) {
            fail(std::string("expected '") + expected + "'");
        }
    }

    Json parse_value() {
        skip_space();
        if (position_ >= input_.size()) {
            fail("unexpected end of input");
        }
        switch (input_[position_]) {
        case 'n':
            expect_literal("null");
            return Json::null();
        case 't':
            expect_literal("true");
            return Json::boolean_value(true);
        case 'f':
            expect_literal("false");
            return Json::boolean_value(false);
        case '"':
            return Json::string_value(parse_string());
        case '[':
            return parse_array();
        case '{':
            return parse_object();
        default:
            if (input_[position_] == '-' || (input_[position_] >= '0' && input_[position_] <= '9')) {
                return parse_number();
            }
            fail("unexpected value");
        }
    }

    void expect_literal(const char *literal) {
        for (const char *character = literal; *character != '\0'; ++character) {
            if (position_ >= input_.size() || input_[position_] != *character) {
                fail("invalid literal");
            }
            ++position_;
        }
    }

    std::string parse_string() {
        expect('"');
        std::string result;
        while (position_ < input_.size()) {
            const char character = input_[position_++];
            if (character == '"') {
                return result;
            }
            if (character == '\\') {
                if (position_ >= input_.size()) {
                    fail("unterminated escape");
                }
                const char escaped = input_[position_++];
                switch (escaped) {
                case '"': result.push_back('"'); break;
                case '\\': result.push_back('\\'); break;
                case '/': result.push_back('/'); break;
                case 'b': result.push_back('\b'); break;
                case 'f': result.push_back('\f'); break;
                case 'n': result.push_back('\n'); break;
                case 'r': result.push_back('\r'); break;
                case 't': result.push_back('\t'); break;
                default: fail("unsupported string escape");
                }
            } else if (static_cast<unsigned char>(character) < 0x20) {
                fail("control character in string");
            } else {
                result.push_back(character);
            }
        }
        fail("unterminated string");
    }

    Json parse_array() {
        expect('[');
        std::vector<Json> values;
        if (consume(']')) {
            return Json::array_value(std::move(values));
        }
        while (true) {
            values.push_back(parse_value());
            if (consume(']')) {
                return Json::array_value(std::move(values));
            }
            expect(',');
        }
    }

    Json parse_object() {
        expect('{');
        std::map<std::string, Json> values;
        if (consume('}')) {
            return Json::object_value(std::move(values));
        }
        while (true) {
            skip_space();
            if (position_ >= input_.size() || input_[position_] != '"') {
                fail("object key must be a string");
            }
            std::string key = parse_string();
            expect(':');
            auto inserted = values.emplace(std::move(key), parse_value());
            if (!inserted.second) {
                fail("duplicate object key");
            }
            if (consume('}')) {
                return Json::object_value(std::move(values));
            }
            expect(',');
        }
    }

    Json parse_number() {
        skip_space();
        const std::size_t begin = position_;
        if (input_[position_] == '-') {
            ++position_;
        }
        if (position_ >= input_.size() || input_[position_] < '0' || input_[position_] > '9') {
            fail("invalid number");
        }
        if (input_[position_] == '0') {
            ++position_;
        } else {
            while (position_ < input_.size() && input_[position_] >= '0' && input_[position_] <= '9') {
                ++position_;
            }
        }
        if (position_ < input_.size() && input_[position_] == '.') {
            ++position_;
            if (position_ >= input_.size() || input_[position_] < '0' || input_[position_] > '9') {
                fail("invalid fraction");
            }
            while (position_ < input_.size() && input_[position_] >= '0' && input_[position_] <= '9') {
                ++position_;
            }
        }
        if (position_ < input_.size() && (input_[position_] == 'e' || input_[position_] == 'E')) {
            ++position_;
            if (position_ < input_.size() && (input_[position_] == '+' || input_[position_] == '-')) {
                ++position_;
            }
            if (position_ >= input_.size() || input_[position_] < '0' || input_[position_] > '9') {
                fail("invalid exponent");
            }
            while (position_ < input_.size() && input_[position_] >= '0' && input_[position_] <= '9') {
                ++position_;
            }
        }
        try {
            return Json::number_value(std::stod(input_.substr(begin, position_ - begin)));
        } catch (const std::exception &) {
            fail("number is out of range");
        }
    }
};

const Json &field(const Json &object, const char *name) {
    if (object.kind != Json::Kind::object) {
        throw std::runtime_error("expected JSON object while reading '" + std::string(name) + "'");
    }
    const auto found = object.object.find(name);
    if (found == object.object.end()) {
        throw std::runtime_error("missing scenario field '" + std::string(name) + "'");
    }
    return found->second;
}

std::string string_field(const Json &object, const char *name) {
    const Json &value = field(object, name);
    if (value.kind != Json::Kind::string) {
        throw std::runtime_error("scenario field '" + std::string(name) + "' must be a string");
    }
    return value.string;
}

std::uint32_t uint_field(const Json &object, const char *name) {
    const Json &value = field(object, name);
    if (value.kind != Json::Kind::number || value.number < 0.0 || value.number > 4294967295.0 ||
        value.number != static_cast<double>(static_cast<std::uint64_t>(value.number))) {
        throw std::runtime_error("scenario field '" + std::string(name) + "' must be a uint32");
    }
    return static_cast<std::uint32_t>(value.number);
}

std::int32_t int_field(const Json &object, const char *name) {
    const Json &value = field(object, name);
    if (value.kind != Json::Kind::number || value.number < -2147483648.0 || value.number > 2147483647.0 ||
        value.number != static_cast<double>(static_cast<std::int64_t>(value.number))) {
        throw std::runtime_error("scenario field '" + std::string(name) + "' must be an int32");
    }
    return static_cast<std::int32_t>(value.number);
}

float float_field(const Json &object, const char *name) {
    const Json &value = field(object, name);
    if (value.kind != Json::Kind::number) {
        throw std::runtime_error("scenario field '" + std::string(name) + "' must be a number");
    }
    return static_cast<float>(value.number);
}

bool bool_field(const Json &object, const char *name) {
    const Json &value = field(object, name);
    if (value.kind != Json::Kind::boolean) {
        throw std::runtime_error("scenario field '" + std::string(name) + "' must be a boolean");
    }
    return value.boolean;
}

std::vector<std::uint32_t> uint_array(const Json &value, const char *name) {
    if (value.kind != Json::Kind::array) {
        throw std::runtime_error("scenario field '" + std::string(name) + "' must be an array");
    }
    std::vector<std::uint32_t> result;
    for (const Json &entry : value.array) {
        if (entry.kind != Json::Kind::number || entry.number < 0.0 || entry.number > 4294967295.0 ||
            entry.number != static_cast<double>(static_cast<std::uint64_t>(entry.number))) {
            throw std::runtime_error("scenario array '" + std::string(name) + "' contains a non-uint32");
        }
        result.push_back(static_cast<std::uint32_t>(entry.number));
    }
    return result;
}

struct Vec3 {
    float x = 0.0f;
    float y = 0.0f;
    float z = 0.0f;
};

Vec3 vec3_field(const Json &object, const char *name) {
    const Json &value = field(object, name);
    if (value.kind != Json::Kind::array || value.array.size() != 3) {
        throw std::runtime_error("scenario field '" + std::string(name) + "' must be a 3-vector");
    }
    for (const Json &entry : value.array) {
        if (entry.kind != Json::Kind::number) {
            throw std::runtime_error("scenario field '" + std::string(name) + "' contains a non-number");
        }
    }
    return {static_cast<float>(value.array[0].number), static_cast<float>(value.array[1].number),
            static_cast<float>(value.array[2].number)};
}

std::optional<std::uint32_t> optional_uint_field(const Json &object, const char *name) {
    const Json &value = field(object, name);
    if (value.kind == Json::Kind::null_value) {
        return std::nullopt;
    }
    if (value.kind != Json::Kind::number || value.number < 0.0 || value.number > 4294967295.0 ||
        value.number != static_cast<double>(static_cast<std::uint64_t>(value.number))) {
        throw std::runtime_error("scenario optional field '" + std::string(name) + "' must be a uint32/null");
    }
    return static_cast<std::uint32_t>(value.number);
}

std::optional<Vec3> optional_vec3_field(const Json &object, const char *name) {
    const Json &value = field(object, name);
    if (value.kind == Json::Kind::null_value) {
        return std::nullopt;
    }
    if (value.kind != Json::Kind::array || value.array.size() != 3) {
        throw std::runtime_error("scenario optional field '" + std::string(name) + "' must be a 3-vector/null");
    }
    for (const Json &entry : value.array) {
        if (entry.kind != Json::Kind::number) {
            throw std::runtime_error("scenario optional field '" + std::string(name) + "' contains a non-number");
        }
    }
    return Vec3{static_cast<float>(value.array[0].number), static_cast<float>(value.array[1].number),
                static_cast<float>(value.array[2].number)};
}

struct Object {
    std::uint32_t id = 0;
    std::string template_name;
    std::string team;
    Vec3 position;
    float orientation = 0.0f;
    float health = 0.0f;
    float max_health = 0.0f;
    std::uint32_t status_bits = 0;
    std::string ai_state;
    std::optional<std::uint32_t> target;
    std::optional<Vec3> target_location;
    float construction_percent = 1.0f;
};

struct Player {
    std::int32_t id = 0;
    std::string name;
    std::string side;
    std::string base_side;
    std::string player_type;
    std::int32_t money = 0;
    std::int32_t power = 0;
    bool low_power = false;
    bool has_radar = false;
    bool is_dead = false;
    std::int32_t rank_level = 0;
    std::int32_t skill_points = 0;
    std::int32_t science_purchase_points = 0;
    std::int32_t total_score = 0;
};

struct Command {
    std::uint32_t frame = 0;
    std::uint32_t player_id = 0;
    std::uint32_t command_id = 0;
    std::string command;
    std::vector<std::uint32_t> selected_units;
};

struct Scenario {
    std::string name;
    std::uint32_t final_frame = 0;
    std::uint32_t rng_base_seed = 0;
    std::uint32_t rng_draws_per_frame = 0;
    std::array<std::uint32_t, 6> declared_rng_seed{};
    std::vector<Object> objects;
    std::vector<Player> players;
    std::vector<Command> commands;
};

Scenario load_scenario(const std::string &path) {
    std::ifstream input(path);
    if (!input) {
        throw std::runtime_error("cannot open scenario '" + path + "'");
    }
    const std::string document((std::istreambuf_iterator<char>(input)), std::istreambuf_iterator<char>());
    const Json root = JsonParser(document).parse();
    if (string_field(root, "schema") != "generals.trace.scenario.v1") {
        throw std::runtime_error("unsupported scenario schema (expected generals.trace.scenario.v1)");
    }

    Scenario scenario;
    scenario.name = string_field(root, "scenario");
    scenario.final_frame = uint_field(root, "final_frame");
    scenario.rng_base_seed = uint_field(root, "rng_base_seed");
    scenario.rng_draws_per_frame = uint_field(root, "rng_draws_per_frame");
    const std::vector<std::uint32_t> seed = uint_array(field(root, "rng_seed"), "rng_seed");
    if (seed.size() != scenario.declared_rng_seed.size()) {
        throw std::runtime_error("rng_seed must contain exactly six uint32 values");
    }
    std::copy(seed.begin(), seed.end(), scenario.declared_rng_seed.begin());

    const Json &objects = field(root, "objects");
    if (objects.kind != Json::Kind::array) {
        throw std::runtime_error("objects must be an array");
    }
    for (const Json &value : objects.array) {
        scenario.objects.push_back(Object{
            uint_field(value, "id"),
            string_field(value, "template"),
            string_field(value, "team"),
            vec3_field(value, "position"),
            float_field(value, "orientation"),
            float_field(value, "health"),
            float_field(value, "max_health"),
            uint_field(value, "status_bits"),
            string_field(value, "ai_state"),
            optional_uint_field(value, "target"),
            optional_vec3_field(value, "target_location"),
            float_field(value, "construction_percent"),
        });
    }

    const Json &players = field(root, "players");
    if (players.kind != Json::Kind::array) {
        throw std::runtime_error("players must be an array");
    }
    for (const Json &value : players.array) {
        scenario.players.push_back(Player{
            int_field(value, "id"), string_field(value, "name"), string_field(value, "side"),
            string_field(value, "base_side"), string_field(value, "player_type"), int_field(value, "money"),
            int_field(value, "power"), bool_field(value, "low_power"), bool_field(value, "has_radar"),
            bool_field(value, "is_dead"), int_field(value, "rank_level"), int_field(value, "skill_points"),
            int_field(value, "science_purchase_points"), int_field(value, "total_score"),
        });
    }

    const Json &commands = field(root, "commands");
    if (commands.kind != Json::Kind::array) {
        throw std::runtime_error("commands must be an array");
    }
    for (const Json &value : commands.array) {
        Command command{uint_field(value, "frame"), uint_field(value, "player_id"), uint_field(value, "command_id"),
                        string_field(value, "command"), uint_array(field(value, "selected_units"), "selected_units")};
        std::sort(command.selected_units.begin(), command.selected_units.end());
        scenario.commands.push_back(std::move(command));
    }
    std::sort(scenario.commands.begin(), scenario.commands.end(), [](const Command &left, const Command &right) {
        return std::tie(left.frame, left.command_id, left.player_id) < std::tie(right.frame, right.command_id, right.player_id);
    });
    return scenario;
}

class CanonicalCrc32 {
public:
    void byte(std::uint8_t value) {
        crc_ ^= value;
        for (int bit = 0; bit < 8; ++bit) {
            crc_ = (crc_ & 1U) ? (crc_ >> 1U) ^ 0xedb88320U : crc_ >> 1U;
        }
    }

    void bytes(const void *data, std::size_t size) {
        const auto *values = static_cast<const std::uint8_t *>(data);
        for (std::size_t index = 0; index < size; ++index) {
            byte(values[index]);
        }
    }

    void u32(std::uint32_t value) {
        for (int shift = 0; shift < 32; shift += 8) {
            byte(static_cast<std::uint8_t>(value >> shift));
        }
    }

    void i32(std::int32_t value) { u32(static_cast<std::uint32_t>(value)); }

    void f32(float value) {
        std::uint32_t bits = 0;
        static_assert(sizeof(bits) == sizeof(value), "float must be 32-bit");
        std::memcpy(&bits, &value, sizeof(bits));
        u32(bits);
    }

    void string_value(const std::string &value) {
        u32(static_cast<std::uint32_t>(value.size()));
        bytes(value.data(), value.size());
    }

    std::uint32_t finish() const { return ~crc_; }

private:
    std::uint32_t crc_ = 0xffffffffU;
};

void hash_vec3(CanonicalCrc32 &crc, const Vec3 &value) {
    crc.f32(value.x);
    crc.f32(value.y);
    crc.f32(value.z);
}

void hash_optional_uint(CanonicalCrc32 &crc, const std::optional<std::uint32_t> &value) {
    crc.byte(value.has_value() ? 1U : 0U);
    if (value) {
        crc.u32(*value);
    }
}

void hash_optional_vec3(CanonicalCrc32 &crc, const std::optional<Vec3> &value) {
    crc.byte(value.has_value() ? 1U : 0U);
    if (value) {
        hash_vec3(crc, *value);
    }
}

std::uint32_t frame_crc(std::uint32_t frame, const std::array<std::uint32_t, 6> &rng_seed,
                        const std::vector<Command> &commands, const Scenario &scenario) {
    CanonicalCrc32 crc;
    crc.bytes("FRAME", 5);
    crc.u32(frame);
    crc.bytes("RNG", 3);
    for (const std::uint32_t seed : rng_seed) {
        crc.u32(seed);
    }
    crc.bytes("COMMANDS", 8);
    crc.u32(static_cast<std::uint32_t>(commands.size()));
    for (const Command &command : commands) {
        crc.u32(command.player_id);
        crc.u32(command.command_id);
        crc.string_value(command.command);
        crc.u32(static_cast<std::uint32_t>(command.selected_units.size()));
        for (const std::uint32_t unit : command.selected_units) {
            crc.u32(unit);
        }
    }
    crc.bytes("OBJECTS", 7);
    crc.u32(static_cast<std::uint32_t>(scenario.objects.size()));
    for (const Object &object : scenario.objects) {
        crc.u32(object.id);
        crc.string_value(object.template_name);
        crc.string_value(object.team);
        hash_vec3(crc, object.position);
        crc.f32(object.orientation);
        crc.f32(object.health);
        crc.f32(object.max_health);
        crc.u32(object.status_bits);
        crc.string_value(object.ai_state);
        hash_optional_uint(crc, object.target);
        hash_optional_vec3(crc, object.target_location);
        crc.f32(object.construction_percent);
    }
    crc.bytes("PLAYERS", 7);
    crc.u32(static_cast<std::uint32_t>(scenario.players.size()));
    for (const Player &player : scenario.players) {
        crc.i32(player.id);
        crc.string_value(player.name);
        crc.string_value(player.side);
        crc.string_value(player.base_side);
        crc.string_value(player.player_type);
        crc.i32(player.money);
        crc.i32(player.power);
        crc.byte(player.low_power ? 1U : 0U);
        crc.byte(player.has_radar ? 1U : 0U);
        crc.byte(player.is_dead ? 1U : 0U);
        crc.i32(player.rank_level);
        crc.i32(player.skill_points);
        crc.i32(player.science_purchase_points);
        crc.i32(player.total_score);
    }
    crc.bytes("EVENTS", 6);
    crc.u32(0);
    crc.bytes("XFER", 4);
    crc.u32(0);
    crc.bytes("VICTORY", 7);
    return crc.finish();
}

void indent(std::ostream &output, int depth) { output << std::string(static_cast<std::size_t>(depth) * 2U, ' '); }

void escaped(std::ostream &output, const std::string &value) {
    output << '"';
    for (const char character : value) {
        switch (character) {
        case '"': output << "\\\""; break;
        case '\\': output << "\\\\"; break;
        case '\n': output << "\\n"; break;
        case '\r': output << "\\r"; break;
        case '\t': output << "\\t"; break;
        default: output << character; break;
        }
    }
    output << '"';
}

void number(std::ostream &output, float value) {
    output << std::setprecision(9) << std::defaultfloat << value;
}

void vec3_json(std::ostream &output, const Vec3 &value) {
    output << '[';
    number(output, value.x); output << ", "; number(output, value.y); output << ", "; number(output, value.z);
    output << ']';
}

void object_json(std::ostream &output, const Object &object, int depth) {
    output << "{\n";
    indent(output, depth + 1); output << "\"id\": " << object.id << ",\n";
    indent(output, depth + 1); output << "\"template\": "; escaped(output, object.template_name); output << ",\n";
    indent(output, depth + 1); output << "\"team\": "; escaped(output, object.team); output << ",\n";
    indent(output, depth + 1); output << "\"position\": "; vec3_json(output, object.position); output << ",\n";
    indent(output, depth + 1); output << "\"orientation\": "; number(output, object.orientation); output << ",\n";
    indent(output, depth + 1); output << "\"health\": "; number(output, object.health); output << ",\n";
    indent(output, depth + 1); output << "\"max_health\": "; number(output, object.max_health); output << ",\n";
    indent(output, depth + 1); output << "\"status_bits\": " << object.status_bits << ",\n";
    indent(output, depth + 1); output << "\"ai_state\": "; escaped(output, object.ai_state); output << ",\n";
    indent(output, depth + 1); output << "\"target\": ";
    if (object.target) output << *object.target; else output << "null";
    output << ",\n";
    indent(output, depth + 1); output << "\"target_location\": ";
    if (object.target_location) vec3_json(output, *object.target_location); else output << "null";
    output << ",\n";
    indent(output, depth + 1); output << "\"construction_percent\": "; number(output, object.construction_percent); output << '\n';
    indent(output, depth); output << '}';
}

void player_json(std::ostream &output, const Player &player, int depth) {
    output << "{\n";
    indent(output, depth + 1); output << "\"id\": " << player.id << ",\n";
    indent(output, depth + 1); output << "\"name\": "; escaped(output, player.name); output << ",\n";
    indent(output, depth + 1); output << "\"side\": "; escaped(output, player.side); output << ",\n";
    indent(output, depth + 1); output << "\"base_side\": "; escaped(output, player.base_side); output << ",\n";
    indent(output, depth + 1); output << "\"player_type\": "; escaped(output, player.player_type); output << ",\n";
    indent(output, depth + 1); output << "\"money\": " << player.money << ",\n";
    indent(output, depth + 1); output << "\"power\": " << player.power << ",\n";
    indent(output, depth + 1); output << "\"low_power\": " << (player.low_power ? "true" : "false") << ",\n";
    indent(output, depth + 1); output << "\"has_radar\": " << (player.has_radar ? "true" : "false") << ",\n";
    indent(output, depth + 1); output << "\"is_dead\": " << (player.is_dead ? "true" : "false") << ",\n";
    indent(output, depth + 1); output << "\"rank_level\": " << player.rank_level << ",\n";
    indent(output, depth + 1); output << "\"skill_points\": " << player.skill_points << ",\n";
    indent(output, depth + 1); output << "\"science_purchase_points\": " << player.science_purchase_points << ",\n";
    indent(output, depth + 1); output << "\"total_score\": " << player.total_score << '\n';
    indent(output, depth); output << '}';
}

void command_json(std::ostream &output, const Command &command, int depth) {
    output << "{\n";
    indent(output, depth + 1); output << "\"player_id\": " << command.player_id << ",\n";
    indent(output, depth + 1); output << "\"command_id\": " << command.command_id << ",\n";
    indent(output, depth + 1); output << "\"command\": "; escaped(output, command.command); output << ",\n";
    indent(output, depth + 1); output << "\"selected_units\": [";
    for (std::size_t index = 0; index < command.selected_units.size(); ++index) {
        if (index != 0) output << ", ";
        output << command.selected_units[index];
    }
    output << "]\n";
    indent(output, depth); output << '}';
}

void frame_json(std::ostream &output, std::uint32_t frame, const std::array<std::uint32_t, 6> &rng_seed,
                const std::vector<Command> &commands, const Scenario &scenario) {
    output << "{\n";
    indent(output, 3); output << "\"frame\": " << frame << ",\n";
    indent(output, 3); output << "\"rng_seed\": [";
    for (std::size_t index = 0; index < rng_seed.size(); ++index) {
        if (index != 0) output << ", ";
        output << rng_seed[index];
    }
    output << "],\n";
    indent(output, 3); output << "\"commands\": [";
    if (!commands.empty()) output << '\n';
    for (std::size_t index = 0; index < commands.size(); ++index) {
        indent(output, 4); command_json(output, commands[index], 4);
        if (index + 1 != commands.size()) output << ',';
        output << '\n';
    }
    indent(output, 3); output << "],\n";
    indent(output, 3); output << "\"objects\": [\n";
    for (std::size_t index = 0; index < scenario.objects.size(); ++index) {
        indent(output, 4); object_json(output, scenario.objects[index], 4);
        if (index + 1 != scenario.objects.size()) output << ',';
        output << '\n';
    }
    indent(output, 3); output << "],\n";
    indent(output, 3); output << "\"players\": [\n";
    for (std::size_t index = 0; index < scenario.players.size(); ++index) {
        indent(output, 4); player_json(output, scenario.players[index], 4);
        if (index + 1 != scenario.players.size()) output << ',';
        output << '\n';
    }
    indent(output, 3); output << "],\n";
    indent(output, 3); output << "\"events\": [],\n";
    indent(output, 3); output << "\"xfer_bytes\": [],\n";
    indent(output, 3); output << "\"victory_state\": null,\n";
    indent(output, 3); output << "\"crc\": " << frame_crc(frame, rng_seed, commands, scenario) << '\n';
    indent(output, 2); output << '}';
}

} // namespace

int main(int argc, char **argv) {
    if (argc != 2) {
        std::cerr << "usage: generalsmd_frame_trace <scenario.json>\n";
        return 2;
    }
    try {
        const Scenario scenario = load_scenario(argv[1]);
        generalsmd_init_logic_random(scenario.rng_base_seed);

        std::cout << "{\n";
        std::cout << "  \"schema\": \"generals.frame_trace.v2\",\n";
        std::cout << "  \"scenario\": "; escaped(std::cout, scenario.name); std::cout << ",\n";
        std::cout << "  \"final_frame\": " << scenario.final_frame << ",\n";
        std::cout << "  \"producer\": \"generalsmd-cpp-original-randomvalue\",\n";
        std::cout << "  \"authority\": {\n";
        std::cout << "    \"rng\": \"GeneralsMD/Code/GameEngine/Source/Common/RandomValue.cpp\",\n";
        std::cout << "    \"objects\": \"fixture-only (full Windows engine unavailable)\",\n";
        std::cout << "    \"players\": \"fixture-only (full Windows engine unavailable)\",\n";
        std::cout << "    \"commands\": \"fixture-only (full Windows engine unavailable)\"\n";
        std::cout << "  },\n";
        std::cout << "  \"frames\": [\n";
        for (std::uint32_t frame = 1; frame <= scenario.final_frame; ++frame) {
            for (std::uint32_t draw = 0; draw < scenario.rng_draws_per_frame; ++draw) {
                generalsmd_draw_logic_random();
            }
            std::array<std::uint32_t, 6> rng_seed{};
            const std::uint32_t *seed = generalsmd_logic_seed();
            std::copy(seed, seed + rng_seed.size(), rng_seed.begin());
            std::vector<Command> commands;
            for (const Command &command : scenario.commands) {
                if (command.frame == frame) commands.push_back(command);
            }
            indent(std::cout, 2);
            frame_json(std::cout, frame, rng_seed, commands, scenario);
            if (frame != scenario.final_frame) std::cout << ',';
            std::cout << '\n';
        }
        std::cout << "  ]\n}\n";
        return 0;
    } catch (const std::exception &error) {
        std::cerr << "generalsmd_frame_trace: " << error.what() << '\n';
        return 1;
    }
}
