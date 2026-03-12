#include "sr_util.h"
#include "wwdebug.h"
#include "camera.h"
#include "matrix4.h"

#ifdef WW3D_DX8

#include <srNode.hpp>
#include <srCamera.hpp>
#include <srMeshModel.hpp>
#include <srGERD.hpp>

/*********************************************************************************************** 
 * Set_SR_Transform -- copies our object transform into a Surrender object                     * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   08/11/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
void Set_SR_Transform(srNode * obj,const Matrix3D & tm)
{

	srMatrix3 srtm;

	obj->setLocation(tm[0][3], tm[1][3], tm[2][3]);

	srtm[0][0] = tm[0][0];
	srtm[0][1] = tm[0][1];
	srtm[0][2] = tm[0][2];

	srtm[1][0] = tm[1][0];
	srtm[1][1] = tm[1][1];
	srtm[1][2] = tm[1][2];

	srtm[2][0] = tm[2][0];
	srtm[2][1] = tm[2][1];
	srtm[2][2] = tm[2][2];

	obj->setRotation(srtm);
}


/***********************************************************************************************
 * Get_SR_Transform -- Creates a Matrix3D from a surrender object transform                    *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/3/97    GTH : Created.                                                                 *
 *=============================================================================================*/
Matrix3D Get_SR_Transform(srNode * obj)
{
	Matrix3D tm;
	srVector3 pos = obj->getLocation();
	srMatrix3 rot;
	obj->getRotation(rot);

	tm[0][3] = pos[0];
	tm[1][3]	= pos[1];
	tm[2][3]	= pos[2];

	tm[0][0] = rot[0][0];
	tm[0][1] = rot[0][1];
	tm[0][2] = rot[0][2];

	tm[1][0] = rot[1][0];
	tm[1][1] = rot[1][1];
	tm[1][2] = rot[1][2];

	tm[2][0] = rot[2][0];
	tm[2][1] = rot[2][1];
	tm[2][2] = rot[2][2];

	return(tm);
}

/*********************************************************************************************** 
 * Set_SR_Camera_Transform -- copies our camera transform into a Surrender object              * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   08/11/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
void Set_SR_Camera_Transform(srCamera * obj,const Matrix3D & transform)
{
	srMatrix3 srtm;
	Matrix3D tm = transform;

	obj->setLocation(tm[0][3], tm[1][3], tm[2][3]);

	srtm[0][0] = tm[0][0];
	srtm[0][1] = tm[0][1];
	srtm[0][2] = -tm[0][2];

	srtm[1][0] = tm[1][0];
	srtm[1][1] = tm[1][1];
	srtm[1][2] = -tm[1][2];

	srtm[2][0] = tm[2][0];
	srtm[2][1] = tm[2][1];
	srtm[2][2] = -tm[2][2];

	obj->setRotation(srtm);
}


/***********************************************************************************************
 * Get_SR_Camera_Transform -- creates a Matrix3D from a surrender camera transform             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/3/97    GTH : Created.                                                                 *
 *=============================================================================================*/
Matrix3D Get_SR_Camera_Transform(srCamera * obj)
{
	Matrix3D tm;

	tm[0][3] = obj->getLocationX();
	tm[1][3]	= obj->getLocationY();
	tm[2][3]	= obj->getLocationZ();

	srMatrix3 srtm;
	obj->getRotation(srtm);

	tm[0][0] = srtm[0][0];
	tm[0][1] = srtm[0][1];
	tm[0][2] = -srtm[0][2];

	tm[1][0] = srtm[1][0];
	tm[1][1] = srtm[1][1];
	tm[1][2] = -srtm[1][2];

	tm[2][0] = srtm[2][0];
	tm[2][1] = srtm[2][1];
	tm[2][2] = -srtm[2][2];

	return tm;
}


/***********************************************************************************************
 * Push_Multiply_Westwood_Matrix -- push and multiply a matrix into the gerd                   *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   12/10/98   GTH : Created.                                                                 *
 *=============================================================================================*/
void Push_Multiply_Westwood_Matrix(srGERD * gerd,const Matrix3D & tm)
{
	WWASSERT(gerd);
	gerd->matrixMode(srGERD::MODELVIEW);
	
	srMatrix4x3 srtm;
	Convert_Westwood_Matrix(tm,&srtm);
	gerd->pushMultMatrix(srtm);
}


void Convert_Westwood_Matrix(const Matrix3D & tm,srMatrix4 * set_sr_tm)
{
	(*set_sr_tm)[0][0] = tm[0][0];
	(*set_sr_tm)[0][1] = tm[0][1];
	(*set_sr_tm)[0][2] = tm[0][2];
	(*set_sr_tm)[0][3] = tm[0][3];

	(*set_sr_tm)[1][0] = tm[1][0];
	(*set_sr_tm)[1][1] = tm[1][1];
	(*set_sr_tm)[1][2] = tm[1][2];
	(*set_sr_tm)[1][3] = tm[1][3];

	(*set_sr_tm)[2][0] = tm[2][0];
	(*set_sr_tm)[2][1] = tm[2][1];
	(*set_sr_tm)[2][2] = tm[2][2];
	(*set_sr_tm)[2][3] = tm[2][3];

	(*set_sr_tm)[3][0] = 0.0f;
	(*set_sr_tm)[3][1] = 0.0f;
	(*set_sr_tm)[3][2] = 0.0f;
	(*set_sr_tm)[3][3] = 1.0f;
}

void Convert_Westwood_Matrix(const Matrix3D & wtm,srMatrix4d * set_sr_tm)
{
	(*set_sr_tm)[0][0] = wtm[0][0];
	(*set_sr_tm)[0][1] = wtm[0][1];
	(*set_sr_tm)[0][2] = wtm[0][2];
	(*set_sr_tm)[0][3] = wtm[0][3];

	(*set_sr_tm)[1][0] = wtm[1][0];
	(*set_sr_tm)[1][1] = wtm[1][1];
	(*set_sr_tm)[1][2] = wtm[1][2];
	(*set_sr_tm)[1][3] = wtm[1][3];

	(*set_sr_tm)[2][0] = wtm[2][0];
	(*set_sr_tm)[2][1] = wtm[2][1];
	(*set_sr_tm)[2][2] = wtm[2][2];
	(*set_sr_tm)[2][3] = wtm[2][3];

	(*set_sr_tm)[3][0] = 0.0;
	(*set_sr_tm)[3][1] = 0.0;
	(*set_sr_tm)[3][2] = 0.0;
	(*set_sr_tm)[3][3] = 1.0;
}

void Convert_Westwood_Matrix(const Matrix3D & wtm,srMatrix4x3 * set_sr_tm)
{
	(*set_sr_tm)[0][0] = wtm[0][0];
	(*set_sr_tm)[0][1] = wtm[0][1];
	(*set_sr_tm)[0][2] = wtm[0][2];
	(*set_sr_tm)[0][3] = wtm[0][3];

	(*set_sr_tm)[1][0] = wtm[1][0];
	(*set_sr_tm)[1][1] = wtm[1][1];
	(*set_sr_tm)[1][2] = wtm[1][2];
	(*set_sr_tm)[1][3] = wtm[1][3];

	(*set_sr_tm)[2][0] = wtm[2][0];
	(*set_sr_tm)[2][1] = wtm[2][1];
	(*set_sr_tm)[2][2] = wtm[2][2];
	(*set_sr_tm)[2][3] = wtm[2][3];
}

void Convert_Westwood_Matrix(const Matrix3D & wtm,srMatrix3 * set_sr_tm,srVector3 * set_sr_translation)
{
	(*set_sr_tm)[0][0] = wtm[0][0];
	(*set_sr_tm)[0][1] = wtm[0][1];
	(*set_sr_tm)[0][2] = wtm[0][2];
	(*set_sr_translation)[0] = wtm[0][3];

	(*set_sr_tm)[1][0] = wtm[1][0];
	(*set_sr_tm)[1][1] = wtm[1][1];
	(*set_sr_tm)[1][2] = wtm[1][2];
	(*set_sr_translation)[1] = wtm[1][3];

	(*set_sr_tm)[2][0] = wtm[2][0];
	(*set_sr_tm)[2][1] = wtm[2][1];
	(*set_sr_tm)[2][2] = wtm[2][2];
	(*set_sr_translation)[2] = wtm[2][3];
}

void Convert_Westwood_Matrix(const Matrix4 & wtm,srMatrix4 * set_sr_tm)
{
	(*set_sr_tm)[0][0] = wtm[0][0];
	(*set_sr_tm)[0][1] = wtm[0][1];
	(*set_sr_tm)[0][2] = wtm[0][2];
	(*set_sr_tm)[0][3] = wtm[0][3];

	(*set_sr_tm)[1][0] = wtm[1][0];
	(*set_sr_tm)[1][1] = wtm[1][1];
	(*set_sr_tm)[1][2] = wtm[1][2];
	(*set_sr_tm)[1][3] = wtm[1][3];

	(*set_sr_tm)[2][0] = wtm[2][0];
	(*set_sr_tm)[2][1] = wtm[2][1];
	(*set_sr_tm)[2][2] = wtm[2][2];
	(*set_sr_tm)[2][3] = wtm[2][3];

	(*set_sr_tm)[3][0] = wtm[3][0];
	(*set_sr_tm)[3][1] = wtm[3][1];
	(*set_sr_tm)[3][2] = wtm[3][2];
	(*set_sr_tm)[3][3] = wtm[3][3];
}

void Convert_Surrender_Matrix(const srMatrix4 & srtm,Matrix3D * set_w3d_tm)
{
	(*set_w3d_tm)[0][0] = srtm[0][0];
	(*set_w3d_tm)[0][1] = srtm[0][1];
	(*set_w3d_tm)[0][2] = srtm[0][2];
	(*set_w3d_tm)[0][3] = srtm[0][3];

	(*set_w3d_tm)[1][0] = srtm[1][0];
	(*set_w3d_tm)[1][1] = srtm[1][1];
	(*set_w3d_tm)[1][2] = srtm[1][2];
	(*set_w3d_tm)[1][3] = srtm[1][3];

	(*set_w3d_tm)[2][0] = srtm[2][0];
	(*set_w3d_tm)[2][1] = srtm[2][1];
	(*set_w3d_tm)[2][2] = srtm[2][2];
	(*set_w3d_tm)[2][3] = srtm[2][3];
}

void Convert_Surrender_Matrix(const srMatrix4x3 & srtm,Matrix3D * set_w3d_tm)
{
	(*set_w3d_tm)[0][0] = srtm[0][0];
	(*set_w3d_tm)[0][1] = srtm[0][1];
	(*set_w3d_tm)[0][2] = srtm[0][2];
	(*set_w3d_tm)[0][3] = srtm[0][3];

	(*set_w3d_tm)[1][0] = srtm[1][0];
	(*set_w3d_tm)[1][1] = srtm[1][1];
	(*set_w3d_tm)[1][2] = srtm[1][2];
	(*set_w3d_tm)[1][3] = srtm[1][3];

	(*set_w3d_tm)[2][0] = srtm[2][0];
	(*set_w3d_tm)[2][1] = srtm[2][1];
	(*set_w3d_tm)[2][2] = srtm[2][2];
	(*set_w3d_tm)[2][3] = srtm[2][3];
}

void Multiply_Westwood_And_Surrender_Matrix(const Matrix3D& n,const srMatrix4& m,srMatrix4& srtm_d)
{
	srtm_d[0].make(m[0][0]*n[0][0] + m[0][1]*n[1][0] + m[0][2]*n[2][0],
			  m[0][0]*n[0][1] + m[0][1]*n[1][1] + m[0][2]*n[2][1],
			  m[0][0]*n[0][2] + m[0][1]*n[1][2] + m[0][2]*n[2][2],
			  m[0][0]*n[0][3] + m[0][1]*n[1][3] + m[0][2]*n[2][3] + m[0][3]);

	srtm_d[1].make(m[1][0]*n[0][0] + m[1][1]*n[1][0] + m[1][2]*n[2][0],
			  m[1][0]*n[0][1] + m[1][1]*n[1][1] + m[1][2]*n[2][1],
			  m[1][0]*n[0][2] + m[1][1]*n[1][2] + m[1][2]*n[2][2],
			  m[1][0]*n[0][3] + m[1][1]*n[1][3] + m[1][2]*n[2][3] + m[1][3]);

	srtm_d[2].make(m[2][0]*n[0][0] + m[2][1]*n[1][0] + m[2][2]*n[2][0],
			  m[2][0]*n[0][1] + m[2][1]*n[1][1] + m[2][2]*n[2][1],
			  m[2][0]*n[0][2] + m[2][1]*n[1][2] + m[2][2]*n[2][2],
			  m[2][0]*n[0][3] + m[2][1]*n[1][3] + m[2][2]*n[2][3] + m[2][3]);

	srtm_d[3].make(m[3][0]*n[0][0] + m[3][1]*n[1][0] + m[3][2]*n[2][0],
			  m[3][0]*n[0][1] + m[3][1]*n[1][1] + m[3][2]*n[2][1],
			  m[3][0]*n[0][2] + m[3][1]*n[1][2] + m[3][2]*n[2][2],
			  m[3][0]*n[0][3] + m[3][1]*n[1][3] + m[3][2]*n[2][3] + m[3][3]);
}

/**************************************************************************
 * Get_Camera_Frustum_Corners -- Returns 8 camera frustum corner points.  * 
 *                                                                        * 
 * INPUT:	CameraClass * camera - camera.                                * 
 *                                                                        * 
 * OUTPUT:	(parameter) Vector3 points[8] - array of corner points.       * 
 *                                                                        * 
 * WARNINGS:	points must be an array of length 8 at least, or bad       *
 *             things will happen (this is not checked by the compiler).  * 
 *                                                                        * 
 * HISTORY:                                                               * 
 *   01/18/1998 NH  : Created.                                            * 
 *   04/13/1998 NH  : Modified for SR 1.3.                                * 
 *========================================================================*/
void Get_Camera_Frustum_Corners(const CameraClass * camera, Vector3 points[8])
{
	// Generate the camera-space frustum corner points by linearly
   // extrapolating the viewplane to the near and far z clipping planes.

	// The camera frustum corner points are defined in the following order:
	// When looking at the frustum from the position of the camera, the near four points are
	// numbered: upper left 0, upper right 1, lower left 2, lower right 3. The far plane's
	// points are numbered from 4 to 7 in an analogous fashion.
   // (remember: the camera space has x going to the right, y up and z backwards).
   Vector2 vpmin, vpmax;
   double znear, zfar;
   camera->Get_View_Plane(vpmin, vpmax); // Normalized view plane at a depth of 1.0
   camera->Get_Clip_Planes(znear, zfar);

   // Forward is negative Z in our viewspace coordinate system.
   znear = -znear;
   zfar = -zfar;

   points[0].Set(vpmin.X, vpmax.Y, 1.0);
   points[4] = points[0];
   points[0] *= znear;
   points[4] *= zfar;
   points[1].Set(vpmax.X, vpmax.Y, 1.0);
   points[5] = points[1];
   points[1] *= znear;
   points[5] *= zfar;
   points[2].Set(vpmin.X, vpmin.Y, 1.0);
   points[6] = points[2];
   points[2] *= znear;
   points[6] *= zfar;
   points[3].Set(vpmax.X, vpmin.Y, 1.0);
   points[7] = points[3];
   points[3] *= znear;
   points[7] *= zfar;

	// Transform the eight corners of the view frustum from camera space to world space.
   Matrix3D cam_mat = camera->Get_Transform();
	for (int i = 0; i < 8; i++) {
		Matrix3D::Transform_Vector(cam_mat, points[i], &(points[i]));
	}

}


/**************************************************************************
 * Get_ZClamped_Camera_Frustum_Corners -- Gets zclamped frustum corners.  * 
 *                                                                        * 
 * INPUT:	CameraClass * camera - camera.                                * 
 *				float minz, maxz - depth clamps on frustum.                   * 
 *                                                                        * 
 * OUTPUT:	(parameter) Vector3 points[8] - array of corner points.       * 
 *				returns false of the intersection between the clamped range   *
 *				and the frustum is empty.                                     * 
 *                                                                        * 
 * WARNINGS:	points must be an array of length 8 at least, or bad       *
 *             things will happen (this is not checked by the compiler).  * 
 *                                                                        * 
 * HISTORY:                                                               * 
 *   11/18/1998 NH  : Re-Created.                                         * 
 *========================================================================*/
bool Get_ZClamped_Camera_Frustum_Corners(const CameraClass * camera,
	Vector3 points[8], float minz, float maxz)
{
	int i;

   // (remember: the camera space has x going to the right, y up and z backwards).
   Vector2 vpmin, vpmax;
   double znear, zfar;
   camera->Get_View_Plane(vpmin, vpmax); // Normalized view plane at a depth of 1.0
   camera->Get_Clip_Planes(znear, zfar);

	// Clamp znear, zfar by minz, maxz:
	if (minz > zfar || maxz < znear) {
		return false;
	}
	float startz = minz > znear ? minz : znear;
	float endz = maxz < zfar ? maxz : zfar;
	

   // Forward is negative Z in our viewspace coordinate system.
	znear = -startz;
	zfar = -endz;

   points[0].Set(vpmin.X, vpmax.Y, 1.0);
   points[4] = points[0];
   points[0] *= znear;
   points[4] *= zfar;
   points[1].Set(vpmax.X, vpmax.Y, 1.0);
   points[5] = points[1];
   points[1] *= znear;
   points[5] *= zfar;
   points[2].Set(vpmin.X, vpmin.Y, 1.0);
   points[6] = points[2];
   points[2] *= znear;
   points[6] *= zfar;
   points[3].Set(vpmax.X, vpmin.Y, 1.0);
   points[7] = points[3];
   points[3] *= znear;
   points[7] *= zfar;

	// Transform the eight corners of the view frustum from camera space to world space.
   Matrix3D cam_mat = camera->Get_Transform();
	for (i = 0; i < 8; i++) {
		Matrix3D::Transform_Vector(cam_mat, points[i], &(points[i]));
	}

	return true;
}



#endif //WW3D_DX8
