#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __SOUNDCULLOBJ_H
#define __SOUNDCULLOBJ_H

#include "soundsceneobj.h"
#include "cullsys.h"
#include "refcount.h"
#include "mempool.h"
#include "multilist.h"


/////////////////////////////////////////////////////////////////////////////
//
//	SoundCullObjClass
//
//	Simple 'sound physics' object that wraps a SoundClass object and is derived
// from PhysClass so it can be used with the different culling systems.
//
class SoundCullObjClass : public MultiListObjectClass, public CullableClass
{
	public:

		//////////////////////////////////////////////////////////////////////
		//	Public constructors/destructors
		//////////////////////////////////////////////////////////////////////
		SoundCullObjClass (void)
			: m_SoundObj (NULL),
			  m_Transform (1) {}

		virtual ~SoundCullObjClass (void) { REF_PTR_RELEASE (m_SoundObj); }

		//////////////////////////////////////////////////////////////////////
		//	Get the 'bounds' of this sound
		//////////////////////////////////////////////////////////////////////
		virtual const AABoxClass & Get_Bounding_Box (void) const;

		//////////////////////////////////////////////////////////////////////
		//	Access to the Position/Orientation state of the object
		//////////////////////////////////////////////////////////////////////
		virtual const Matrix3D &	Get_Transform (void) const;
		virtual void					Set_Transform (const Matrix3D &transform);

		//////////////////////////////////////////////////////////////////////
		//	Timestep methods
		//////////////////////////////////////////////////////////////////////
		virtual void					Timestep (float dt) {}

		//////////////////////////////////////////////////////////////////////
		//	Sound object wrapping
		//////////////////////////////////////////////////////////////////////
		virtual void						Set_Sound_Obj (SoundSceneObjClass *sound_obj);
		virtual SoundSceneObjClass *	Peek_Sound_Obj (void) const					{ return m_SoundObj; }

	protected:
		
		//////////////////////////////////////////////////////////////////////
		//	Protected methods
		//////////////////////////////////////////////////////////////////////		

	private:

		//////////////////////////////////////////////////////////////////////
		//	Private member data
		//////////////////////////////////////////////////////////////////////
		SoundSceneObjClass *		m_SoundObj;
		mutable Matrix3D			m_Transform;
		mutable AABoxClass		m_AABox;
};


__inline const Matrix3D &
SoundCullObjClass::Get_Transform (void) const
{
	// Determine the transform to use
	if (m_SoundObj != NULL) {
		m_Transform = m_SoundObj->Get_Transform ();
	}

	// Return a reference to the matrix
	return m_Transform;
}


__inline void
SoundCullObjClass::Set_Transform (const Matrix3D &transform)
{
	m_Transform = transform;

	// Pass the tranform on
	if (m_SoundObj != NULL) {
		m_SoundObj->Set_Transform (m_Transform);
		Set_Cull_Box (Get_Bounding_Box ());
	}

	return ;
}


__inline void
SoundCullObjClass::Set_Sound_Obj (SoundSceneObjClass *sound_obj)
{
	// Start using this sound object
	REF_PTR_SET (m_SoundObj, sound_obj);
	//m_SoundObj =  sound_obj;
	if (m_SoundObj != NULL) {
		m_Transform = m_SoundObj->Get_Transform ();
		Set_Cull_Box (Get_Bounding_Box ());
	}

	return ;
}


__inline const AABoxClass &
SoundCullObjClass::Get_Bounding_Box (void) const
{
	// Get the 'real' values from the 
	if (m_SoundObj != NULL) {
		m_Transform = m_SoundObj->Get_Transform ();
		m_AABox.Extent.X = m_SoundObj->Get_DropOff_Radius ();
		m_AABox.Extent.Y = m_SoundObj->Get_DropOff_Radius ();
		m_AABox.Extent.Z = m_SoundObj->Get_DropOff_Radius ();
	} else {
		m_AABox.Extent.X = 0;
		m_AABox.Extent.Y = 0;
		m_AABox.Extent.Z = 0;
	}

	m_AABox.Center = m_Transform.Get_Translation ();
	return m_AABox;
}

#endif //__SOUNDCULLOBJ_H
