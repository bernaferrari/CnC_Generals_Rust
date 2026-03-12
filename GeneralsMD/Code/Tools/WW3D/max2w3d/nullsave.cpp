#include "nullsave.h"


NullSaveClass::NullSaveClass
(
	char * mesh_name,
	char * container_name,
	Progress_Meter_Class & meter
)
{
	//////////////////////////////////////////////////////////////////////
	// Set up the NullObject description
	//////////////////////////////////////////////////////////////////////
	memset(&NullData,0,sizeof(NullData));

	NullData.Version = W3D_NULL_OBJECT_CURRENT_VERSION;
	if ((container_name != NULL) && (strlen(container_name) > 0)) {
		strcpy(NullData.Name,container_name);
		strcat(NullData.Name,".");
	}
	strcat(NullData.Name,mesh_name);
}



int NullSaveClass::Write_To_File(ChunkSaveClass & csave)
{
	csave.Begin_Chunk(W3D_CHUNK_NULL_OBJECT);
	csave.Write(&NullData,sizeof(NullData));
	csave.End_Chunk();
	return 0;
}



