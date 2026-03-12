#if defined(_MSC_VER)
#pragma once
#endif

#ifndef SHATTERSYSTEM_H
#define SHATTERSYSTEM_H

#include "always.h"


class MeshClass;
class PhysicsSceneClass;
class RenderObjClass;
class Vector3;
class Matrix3D;
class MeshMtlParamsClass;


/**
** ShatterSystem
** This collection of static functions is used to "shatter" a mesh into fragments and
** create projectile objects which use the generated fragments as their render object.
*/
class ShatterSystem
{
public:

	/*
	** Init and Shutdown, these are called from the main physics system init and
	** shutdown.  The BSP shatter planes are loaded up and initialized in Init
	** and released in Shutdown.
	*/
	static void		Init(void);
	static void		Shutdown(void);

	/*
	** Pass in the mesh you want shattered and the scene that you want
	** the shards to be placed in.
	*/
	static void		Shatter_Mesh(MeshClass * mesh,const Vector3 & point,const Vector3 & velocity);

	/*
	** Use these methods to get access to the resultant mesh fragments
	** Get_Fragment_Count - returns the number of meshes created
	** Get_Fragment - returns a pointer (ref-counted!) to the 'n'th mesh 
	** Relese_Fragments - call this when you are done, it causes the ShatterSystem 
	**                    to release its references to the fragments.
	*/
	static int		Get_Fragment_Count(void);
	static RenderObjClass *	Get_Fragment(int fragment_index);
	static RenderObjClass *	Peek_Fragment(int fragment_index);
	static void		Release_Fragments(void);

protected:

	static void		Reset_Clip_Pools(void);
	static void		Process_Clip_Pools(const Matrix3D &Mshatter_to_mesh,MeshClass * mesh,MeshMtlParamsClass & mtl_params);


};


#endif


