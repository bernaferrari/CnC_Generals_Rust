#include "MeshDeformSaveSet.H"
#include "Util.H"


////////////////////////////////////////////////////////////////////////
//
//	Reset
//
////////////////////////////////////////////////////////////////////////
void
MeshDeformSaveSetClass::Reset (void)
{
	//
	//	Free all the keyframe pointers in our list
	//
	for (int index = 0; index < m_DeformData.Count (); index ++) {
		SAFE_DELETE (m_DeformData[index]);
	}

	m_DeformData.Delete_All ();
	m_CurrentKeyFrame = NULL;
	return ;
}


////////////////////////////////////////////////////////////////////////
//
//	Begin_Keyframe
//
////////////////////////////////////////////////////////////////////////
void
MeshDeformSaveSetClass::Begin_Keyframe (float state)
{
	//
	//	Allocate a new keyframe structure
	//
	m_CurrentKeyFrame = new KEYFRAME;
	m_CurrentKeyFrame->state = state;

	//
	//	Add this new keyframe to the end of our list
	//
	m_DeformData.Add (m_CurrentKeyFrame);
	return ;
}


////////////////////////////////////////////////////////////////////////
//
//	End_Keyframe
//
////////////////////////////////////////////////////////////////////////
void
MeshDeformSaveSetClass::End_Keyframe (void)
{
	m_CurrentKeyFrame = NULL;
	return ;
}


////////////////////////////////////////////////////////////////////////
//
//	Add_Vert
//
////////////////////////////////////////////////////////////////////////
void
MeshDeformSaveSetClass::Add_Vert
(
	UINT					vert_index,
	const Point3 &		position,
	const VertColor &	color
)
{
	// State OK?
	assert (m_CurrentKeyFrame != NULL);
	if (m_CurrentKeyFrame != NULL) {

		//
		//	Create a structure that will hold the
		//	vertex information.
		//
		DEFORM_DATA data;
		data.vert_index	= vert_index;
		data.position		= position;
		data.color			= color;
		
		//
		//	Add this vertex information to the keyframe list
		//
		m_CurrentKeyFrame->deform_list.Add (data);
	}

	return ;
}


////////////////////////////////////////////////////////////////////////
//
//	Replace_Deform_Data
//
////////////////////////////////////////////////////////////////////////
void
MeshDeformSaveSetClass::Replace_Deform_Data
(
	int										keyframe_index,
	DynamicVectorClass<DEFORM_DATA> &list
)
{
	KEYFRAME *key_frame = m_DeformData[keyframe_index];
	if (key_frame != NULL) {
		
		//
		//	Replace the vertex deformation list for the keyframe
		//
		key_frame->deform_list.Delete_All ();
		key_frame->deform_list = list;
	}

	return ;
}


////////////////////////////////////////////////////////////////////////
//
//	Get_Deform_Count
//
////////////////////////////////////////////////////////////////////////
/*int
MeshDeformSaveSetClass::Get_Deform_Count (void) const
{
	//
	//	Count up all the deform entries for all the keyframes
	//
	int count = 0;
	for (int index = 0; index < m_DeformData.Count (); index ++) {
		KEYFRAME *key_frame = m_DeformData[index];
		if (key_frame != NULL) {
			count += key_frame->deform_list.Count ();
		}
	}

	return count;
}*/


