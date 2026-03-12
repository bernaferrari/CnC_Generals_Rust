#include "v3_rnd.h"
#include "vector2.h"

const float Vector3Randomizer::OOIntMax = 1.0f / (float)INT_MAX;
const float Vector3Randomizer::OOUIntMax = 1.0f / (float)UINT_MAX;
Random3Class Vector3Randomizer::Randomizer;

Vector3SolidBoxRandomizer::Vector3SolidBoxRandomizer(const Vector3 & extents)
{
	Extents.X = MAX(extents.X, 0.0f);
	Extents.Y = MAX(extents.Y, 0.0f);
	Extents.Z = MAX(extents.Z, 0.0f);
}

void Vector3SolidBoxRandomizer::Get_Vector(Vector3 &vector)
{
	vector.X = Get_Random_Float_Minus1_To_1() * Extents.X;
	vector.Y = Get_Random_Float_Minus1_To_1() * Extents.Y;
	vector.Z = Get_Random_Float_Minus1_To_1() * Extents.Z;
}

float Vector3SolidBoxRandomizer::Get_Maximum_Extent(void)
{
	float max = MAX(Extents.X, Extents.Y);
	max = MAX(max, Extents.Z);
	return max;
}

void Vector3SolidBoxRandomizer::Scale(float scale)
{
	scale = MAX(scale, 0.0f);
	Extents.X *= scale;
	Extents.Y *= scale;
	Extents.Z *= scale;
}


Vector3SolidSphereRandomizer::Vector3SolidSphereRandomizer(float radius)
{
	Radius = MAX(radius, 0.0f);
}

void Vector3SolidSphereRandomizer::Get_Vector(Vector3 &vector)
{
	// Generate vectors in a cube and discard the ones not in a sphere
	float rad_squared = Radius * Radius;
	for (;;) {
		vector.X = Get_Random_Float_Minus1_To_1() * Radius;
		vector.Y = Get_Random_Float_Minus1_To_1() * Radius;
		vector.Z = Get_Random_Float_Minus1_To_1() * Radius;
		if (vector.Length2() <= rad_squared) break;
	}
}

float Vector3SolidSphereRandomizer::Get_Maximum_Extent(void)
{
	return Radius;
}

void Vector3SolidSphereRandomizer::Scale(float scale)
{
	scale = MAX(scale, 0.0f);
	Radius *= scale;
}


Vector3HollowSphereRandomizer::Vector3HollowSphereRandomizer(float radius)
{
	Radius = MAX(radius, 0.0f);
}

void Vector3HollowSphereRandomizer::Get_Vector(Vector3 &vector)
{
	// Generate vectors in a 2x2x2 origin-centered cube, discard the ones not in a unit-radius
	// sphere and scale the result to Radius.
	float v_l2;
	for (;;) {
		vector.X = Get_Random_Float_Minus1_To_1();
		vector.Y = Get_Random_Float_Minus1_To_1();
		vector.Z = Get_Random_Float_Minus1_To_1();
		v_l2 = vector.Length2();
		if (v_l2 <= 1.0f && v_l2 > 0.0f) break;
	}

	float scale = Radius * WWMath::Inv_Sqrt(v_l2);

	vector.X *= scale;
	vector.Y *= scale;
	vector.Z *= scale;
}

float Vector3HollowSphereRandomizer::Get_Maximum_Extent(void)
{
	return Radius;
}

void Vector3HollowSphereRandomizer::Scale(float scale)
{
	scale = MAX(scale, 0.0f);
	Radius *= scale;
}


Vector3SolidCylinderRandomizer::Vector3SolidCylinderRandomizer(float extent, float radius)
{
	Extent = MAX(extent, 0.0f);
	Radius = MAX(radius, 0.0f);
}

void Vector3SolidCylinderRandomizer::Get_Vector(Vector3 &vector)
{
	vector.X = Get_Random_Float_Minus1_To_1() * Extent;

	// Generate 2D vectors in a square and discard the ones not in a circle
	Vector2 vec2;
	float rad_squared = Radius * Radius;
	for (;;) {
		vec2.X = Get_Random_Float_Minus1_To_1() * Radius;
		vec2.Y = Get_Random_Float_Minus1_To_1() * Radius;
		if (vec2.Length2() <= rad_squared) break;
	}

	vector.Y = vec2.X;
	vector.Z = vec2.Y;
}

float Vector3SolidCylinderRandomizer::Get_Maximum_Extent(void)
{
	return MAX(Extent, Radius);
}

void Vector3SolidCylinderRandomizer::Scale(float scale)
{
	scale = MAX(scale, 0.0f);
	Extent *= scale;
	Radius *= scale;
}
