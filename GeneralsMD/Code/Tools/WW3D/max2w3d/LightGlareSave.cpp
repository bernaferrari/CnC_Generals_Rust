#include "LightGlareSave.h"
#include "w3d_file.h"
#include "util.h"
#include "w3dappdata.h"
#include "errclass.h"


LightGlareSaveClass::LightGlareSaveClass
(
	char * mesh_name,
	char * container_name,
	INode * inode,
	Matrix3 & exportspace,
	TimeValue curtime,
	Progress_Meter_Class & meter
)
{
	//////////////////////////////////////////////////////////////////////
	// Init the glare info
	//////////////////////////////////////////////////////////////////////
	memset(&GlareData,0,sizeof(GlareData));

	//////////////////////////////////////////////////////////////////////
	// Get the position of the pivot point relative to the given
	// export coordinate system.
	//////////////////////////////////////////////////////////////////////
	
	// Transform the mesh into the desired coordinate system
	Matrix3 node_matrix = inode->GetObjectTM(curtime);
	Matrix3 offset_matrix = node_matrix * Inverse(exportspace);

	GlareData.Position.X = offset_matrix.GetTrans().x;
	GlareData.Position.Y = offset_matrix.GetTrans().y;
	GlareData.Position.Z = offset_matrix.GetTrans().z;
}



int LightGlareSaveClass::Write_To_File(ChunkSaveClass & csave)
{
	csave.Begin_Chunk(W3D_CHUNK_LIGHTGLARE);
	csave.Begin_Chunk(W3D_CHUNK_LIGHTGLARE_INFO);
	csave.Write(&GlareData,sizeof(GlareData));
	csave.End_Chunk();
	csave.End_Chunk();
	return 0;
}



