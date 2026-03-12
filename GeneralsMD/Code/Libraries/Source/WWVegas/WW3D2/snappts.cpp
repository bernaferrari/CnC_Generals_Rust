#include "snappts.h"
#include "chunkio.h"
#include "w3d_file.h"
#include "w3derr.h"

WW3DErrorType SnapPointsClass::Load_W3D(ChunkLoadClass & cload)
{
	// this function assumes that a W3D_CHUNK_POINTS has been opened
	int size = cload.Cur_Chunk_Length();
	int count = size / sizeof (W3dVectorStruct);
	
	Resize(count);
	for (int i=0; i<count; i++) {
		W3dVectorStruct vec;
		if (cload.Read(&vec,sizeof(vec)) != sizeof(vec)) {
			goto Error;
		}

		Vector3 point (vec.X, vec.Y, vec.Z);
		Add (point);
		//(*this)[i].Set(vec.X,vec.Y,vec.X);
	}

	return WW3D_ERROR_OK;

Error:

	return WW3D_ERROR_LOAD_FAILED;
}
