#ifndef __MESH_DEFORM_SAVE_SET_H
#define __MESH_DEFORM_SAVE_SET_H

#include <Max.h>
#include "Vector.H"

// Forward declarations
class ChunkSaveClass;


///////////////////////////////////////////////////////////////////////////
//
//	MeshDeformSaveSetClass
//
///////////////////////////////////////////////////////////////////////////
class MeshDeformSaveSetClass
{
	public:

		//////////////////////////////////////////////////////////////////////
		//	Public friends
		//////////////////////////////////////////////////////////////////////
		friend class MeshDeformSaveClass;


	protected:

	protected:

		//////////////////////////////////////////////////////////////////////
		//	Protected data types
		//////////////////////////////////////////////////////////////////////
		typedef struct _DEFORM_DATA
		{
			UINT			vert_index;
			Point3		position;
			VertColor	color;

			// Don't care, DynamicVectorClass needs these
			bool operator== (const _DEFORM_DATA &src) { return false; }
			bool operator!= (const _DEFORM_DATA &src) { return true; }
		} DEFORM_DATA;

		//////////////////////////////////////////////////////////////////////
		//	Protected data types
		//////////////////////////////////////////////////////////////////////
		typedef struct
		{
			float										state;
			DynamicVectorClass<DEFORM_DATA>	deform_list;
		} KEYFRAME;


public:

		//////////////////////////////////////////////////////////////////////
		//	Public constructors/destructors
		//////////////////////////////////////////////////////////////////////
		MeshDeformSaveSetClass (void)
			:	m_Flags (0),
				m_CurrentKeyFrame (NULL)	{ }
		~MeshDeformSaveSetClass (void)	{ Reset (); }

		//////////////////////////////////////////////////////////////////////
		//	Public methods
		//////////////////////////////////////////////////////////////////////
		
		// Keyframe managment
		void					Begin_Keyframe (float state);
		void					End_Keyframe (void);
		
		// Vertex managment
		void					Add_Vert (UINT vert_index, const Point3 &position, const VertColor &color);

		// Misc
		void					Reset (void);
		bool					Is_Empty (void) const	{ return m_DeformData.Count () == 0; }

		// Flag support
		bool					Get_Flag (unsigned int flag) const				{ return (m_Flags & flag) == flag; }
		void					Set_Flag (unsigned int flag, bool value)		{ if (value) (m_Flags |= flag); else (m_Flags &= ~flag); }
		unsigned int		Get_Flags (void) const								{ return m_Flags; }

		// Enumeration
		float					Get_Deform_State (int key_frame) const			{ return m_DeformData[key_frame]->state; }
		int					Get_Keyframe_Count (void) const					{ return m_DeformData.Count (); }
		int					Get_Deform_Data_Count (int key_frame) const	{ return m_DeformData[key_frame]->deform_list.Count (); }
		DEFORM_DATA &		Get_Deform_Data (int key_frame, int index)	{ return m_DeformData[key_frame]->deform_list[index]; }
		void					Replace_Deform_Data (int keyframe_index, DynamicVectorClass<DEFORM_DATA> &list);

	private:

		//////////////////////////////////////////////////////////////////////
		//	Private member data
		//////////////////////////////////////////////////////////////////////
		DynamicVectorClass<KEYFRAME *>		m_DeformData;
		KEYFRAME *									m_CurrentKeyFrame;
		unsigned int								m_Flags;
};

#endif //__MESH_DEFORM_SAVE_SET_H
