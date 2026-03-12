#include "colmath.h"
#include "aaplane.h"
#include "plane.h"
#include "lineseg.h"
#include "tri.h"
#include "sphere.h"
#include "aabox.h"
#include "obbox.h"
#include "wwdebug.h"



// OBBox functions, where is operand B with respect to the OBBox
CollisionMath::OverlapType
CollisionMath::Overlap_Test(const OBBoxClass & box,const Vector3 & point)
{
	// transform point into box coordinate system
	Vector3 localpoint;
	Matrix3x3::Transpose_Rotate_Vector(box.Basis,(point - box.Center),&localpoint);

	// if the point is outside any of the extents, it is outside the box
	if (WWMath::Fabs(localpoint.X) > box.Extent.X) {
		return OUTSIDE;
	}
	if (WWMath::Fabs(localpoint.Y) > box.Extent.Y) {
		return OUTSIDE;
	}
	if (WWMath::Fabs(localpoint.Z) > box.Extent.Z) {
		return OUTSIDE;
	}
	return INSIDE;
}

CollisionMath::OverlapType
CollisionMath::Overlap_Test(const OBBoxClass & box,const LineSegClass & line)
{
	CastResultStruct res;
	Collide(line,box,&res);
	return eval_overlap_collision(res);
}

CollisionMath::OverlapType
CollisionMath::Overlap_Test(const OBBoxClass & box,const TriClass & tri)
{
	CastResultStruct res;
	Collide(box,Vector3(0,0,0),tri,Vector3(0,0,0),&res);
	return eval_overlap_collision(res);
}

CollisionMath::OverlapType 
CollisionMath::Overlap_Test(const AABoxClass & aabox,const OBBoxClass & obbox)
{
	if (CollisionMath::Intersection_Test(aabox,obbox)) {
		return BOTH;	// inside or overlapping
	} else {
		return OUTSIDE;
	}
}

CollisionMath::OverlapType 
CollisionMath::Overlap_Test(const OBBoxClass & obbox,const AABoxClass & aabox)
{
	if (CollisionMath::Intersection_Test(obbox,aabox)) {
		return BOTH;	// inside or overlapping
	} else {
		return OUTSIDE;
	}
}


CollisionMath::OverlapType
CollisionMath::Overlap_Test(const OBBoxClass & box,const OBBoxClass & box2)
{
	CastResultStruct res;
	Collide(box,Vector3(0,0,0),box2,Vector3(0,0,0),&res);
	return eval_overlap_collision(res);
}

bool CollisionMath::Collide
(
	const OBBoxClass &		box,
	const Vector3 &			move_vector,
	const PlaneClass &		plane,
	CastResultStruct *		result
)
{
	float frac;

	float extent = box.Project_To_Axis(plane.N);
	float dist = Vector3::Dot_Product(plane.N,box.Center) + plane.D;
	float move = Vector3::Dot_Product(plane.N,move_vector);

	if (dist > extent) {
		if (dist + move > extent) {
			// entire move ok!
			frac = 1.0f;
		} else {
			// partial move allowed
			frac = (extent - dist) / move;
		}

	} else if (dist < -extent) {
		if (dist + move < -extent) {
			// entire move ok!
			frac = 1.0f;
		} else {
			// partial move allowed
			frac = (-extent - dist) / move;
		}
	} else {
		result->StartBad = true;
		result->Normal = plane.N;
		return true;
	}

	if (frac < result->Fraction) {
		result->Fraction = frac;
		result->Normal = plane.N;
		if (result->ComputeContactPoint) {

			Vector3 move_dir(move_vector);
			move_dir.Normalize();
			float move_extent = Vector3::Dot_Product(move_dir,box.Extent);
			result->ContactPoint = box.Center + result->Fraction*move_vector  + move_extent*move_dir;

		}
		return true;
	}
	return false;
}

