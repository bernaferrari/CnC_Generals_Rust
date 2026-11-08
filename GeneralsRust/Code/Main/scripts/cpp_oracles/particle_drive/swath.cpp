// Portable transcription of ParticleUplinkCannonUpdate.cpp:483-529 and
// WWMath vector2.h:280-287, matrix3d.h:827-845. No retail assets embedded.
// Build: c++ -std=c++17 -O0 -ffp-contract=off swath.cpp -o /tmp/puc-swath
// Input: building xyz, initial target xyz, now birth death, distance amplitude.
// Coordinates are the original x/y ground plane and z height. Output: x y.
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>

int main() {
    float bx, by, bz, tx, ty, tz, distance, amplitude;
    unsigned now, birth, death;
    while (std::cin >> bx >> by >> bz >> tx >> ty >> tz >> now >> birth >> death >> distance >> amplitude) {
        const float pi = 3.14159265358979323846f;
        const float two_pi = 2.0f * pi;
        float factor = float(now - birth) / float(death - birth);
        float radians = factor * two_pi - pi;
        float cxDistance = factor * distance - distance * 0.5f;
        float height = std::sin(double(radians));
        float cxHeight = height * amplitude;
        float dx = tx - bx, dy = ty - by, dz = tz - bz;
        float targetDistance = std::sqrt((dx * dx + dy * dy) + dz * dz);
        float x = cxDistance + targetDistance, y = cxHeight;
        targetDistance = std::sqrt((x * x + y * y) + 0.0f);
        auto normalize = [](float &a, float &b) {
            float len2 = a * a + b * b;
            if (len2 != 0.0f) {
                float oolen = 1.0f / std::sqrt(len2);
                a *= oolen; b *= oolen;
            }
        };
        normalize(dx, dy);
        normalize(x, y);
        float dot = std::min(0.99999f, std::max(-0.99999f, dx * x + dy * y));
        // Trig.cpp defines DEFAULT_TRIG, so ACos calls acosf.
        float angle = std::acos(dot);
        if (dy >= 0.0f) angle = -angle;
        float c = std::cos(-angle), sine = std::sin(-angle);
        float row00 = 1.0f * targetDistance, row01 = 0.0f * targetDistance;
        float row10 = 0.0f * targetDistance, row11 = 1.0f * targetDistance;
        std::cout << std::setprecision(9) << bx + (c * row00 + sine * row01)
                  << ' ' << by + (c * row10 + sine * row11) << '\n';
    }
}
