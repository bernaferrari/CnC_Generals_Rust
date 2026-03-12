#include "dazzlesave.h"
#include "w3d_file.h"
#include "util.h"
#include "w3dappdata.h"
#include "errclass.h"


DazzleSaveClass::DazzleSaveClass
(
	char * mesh_name,
	char * container_name,
	INode * inode,
	Matrix3 & exportspace,
	TimeValue curtime,
	Progress_Meter_Class & meter
) 
{
	assert(mesh_name != NULL);
	assert(container_name != NULL);

	/*
	** Set up the render object name
	*/
	memset(&W3DName,0,sizeof(W3DName));
	if ((container_name != NULL) && (strlen(container_name) > 0)) {
		strcpy(W3DName,container_name);
		strcat(W3DName,".");
	}
	strcat(W3DName,mesh_name);

	/*
	** Dig the dazzle-type out of the appropriate App-Data chunk on
	** the INode.
	*/
	W3DDazzleAppDataStruct * dazzle_data = W3DDazzleAppDataStruct::Get_App_Data(inode);
	strncpy(DazzleType,dazzle_data->DazzleType,sizeof(DazzleType));
}



int DazzleSaveClass::Write_To_File(ChunkSaveClass & csave)
{
	csave.Begin_Chunk(W3D_CHUNK_DAZZLE);

	csave.Begin_Chunk(W3D_CHUNK_DAZZLE_NAME);
	csave.Write(W3DName,strlen(W3DName) + 1);
	csave.End_Chunk();

	csave.Begin_Chunk(W3D_CHUNK_DAZZLE_TYPENAME);
	csave.Write(DazzleType,strlen(DazzleType) + 1);
	csave.End_Chunk();
	
	csave.End_Chunk();
	return 0;
}



