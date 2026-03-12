#ifndef PROJECTOR_H
#define PROJECTOR_H

#include "always.h"
#include "matrix3d.h"
#include "matrix4.h"
#include "aabox.h"
#include "obbox.h"


class MatrixMapperClass;


/**
** ProjectorClass
** This is a class which encapsulates the data needed to describe a projection.  It isn't
** really useful by itself but it is a common base class between TexProjectClass and DecalGeneratorClass.
*/
class ProjectorClass
{
public:
	
	ProjectorClass(void);
	virtual ~ProjectorClass(void);

	virtual void					Set_Transform(const Matrix3D & tm);
	virtual const Matrix3D &	Get_Transform(void) const;

	virtual void					Set_Perspective_Projection(float hfov,float vfov,float znear,float zfar);
	virtual void					Set_Ortho_Projection(float xmin,float xmax,float ymin,float ymax,float znear,float zfar);

	const OBBoxClass &			Get_Bounding_Volume(void) const { return WorldBoundingVolume; }
	void								Compute_Texture_Coordinate(const Vector3 & point,Vector3 * set_stq);

	MatrixMapperClass*			Peek_Mapper() const { return Mapper; }

protected:

	virtual void					Update_WS_Bounding_Volume(void);

	Matrix3D							Transform;
	Matrix4x4						Projection;

	AABoxClass						LocalBoundingVolume;
	OBBoxClass						WorldBoundingVolume;

	MatrixMapperClass *			Mapper;
};


#endif


