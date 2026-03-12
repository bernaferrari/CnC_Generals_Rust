#include "shdloader.h"
#include "shdmesh.h"
#include "mesh.h"

ShdMeshLoaderClass			_ShdMeshLoader;
ShdMeshLegacyLoaderClass	_ShdMeshLegacyLoader;

/***********************************************************************************************
 * ShdMeshLoaderClass::Load -- reads in a shader mesh and creates a prototype for it           *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   5/21/02    KJM : Created.                                                                 *
 *=============================================================================================*/
PrototypeClass* ShdMeshLoaderClass::Load_W3D(ChunkLoadClass& cload)
{
	ShdMeshClass* mesh=NEW_REF(ShdMeshClass, ());

	if (mesh==NULL) 
	{
		return NULL;
	}

	if (mesh->Load_W3D(cload)!=WW3D_ERROR_OK) 
	{
		// if the load failed, delete the mesh
		assert(mesh->Num_Refs() == 1);
		mesh->Release_Ref();
		return NULL;

	}

	// create the prototype and add it to the lists
	PrimitivePrototypeClass * newproto = new PrimitivePrototypeClass(mesh);
	mesh->Release_Ref();
	return newproto;
}


PrototypeClass * ShdMeshLegacyLoaderClass::Load_W3D(ChunkLoadClass & cload)
{
	MeshClass * mesh = NEW_REF( MeshClass, () );

	if (mesh == NULL) {
		return NULL;
	}

	if (mesh->Load_W3D(cload) != WW3D_ERROR_OK) {

		// if the load failed, delete the mesh
		assert(mesh->Num_Refs() == 1);
		mesh->Release_Ref();
		return NULL;

	} else {
		ShdMeshClass* shdmesh=NEW_REF( ShdMeshClass, () );
		shdmesh->Init_From_Legacy_Mesh(mesh);
		mesh->Release_Ref();

		// create the prototype and add it to the lists
		PrimitivePrototypeClass * newproto = W3DNEW PrimitivePrototypeClass(shdmesh);
		shdmesh->Release_Ref();
		return newproto;
	
	}
}
