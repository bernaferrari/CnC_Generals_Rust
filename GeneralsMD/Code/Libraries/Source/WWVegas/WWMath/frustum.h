#if defined(_MSC_VER)
#pragma once
#endif

#ifndef FRUSTUM_H
#define FRUSTUM_H

#include "vector3.h"
#include "plane.h"


class FrustumClass
{
public:
	void Init(			const Matrix3D & camera,
							const Vector2 & viewport_min,
							const Vector2 & viewport_max,
							float znear,
							float zfar );

	const Vector3 &	Get_Bound_Min(void) const		{ return BoundMin; }
	const Vector3 &	Get_Bound_Max(void) const		{ return BoundMax; }

public:

	Matrix3D				CameraTransform;
	PlaneClass			Planes[6];
	Vector3				Corners[8];
	Vector3				BoundMin;
	Vector3				BoundMax;
};



#endif

