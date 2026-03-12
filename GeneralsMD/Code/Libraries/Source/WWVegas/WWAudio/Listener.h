#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __LISTENER_H
#define __LISTENER_H


#include "Sound3D.H"


/////////////////////////////////////////////////////////////////////////////////
//
//	Listener3DClass
//
//	Class defining the 'listeners' 3D position/velocity in the world.  This should
// only be used by the SoundSceneClass.
//
class Listener3DClass : public Sound3DClass
{
	public:

		//////////////////////////////////////////////////////////////////////
		//	Friend classes
		//////////////////////////////////////////////////////////////////////
		friend class SoundSceneClass;

		//////////////////////////////////////////////////////////////////////
		//	Public constructors/destructors
		//////////////////////////////////////////////////////////////////////
		Listener3DClass (void);
		virtual ~Listener3DClass (void);

		//////////////////////////////////////////////////////////////////////
		//	Identification methods
		//////////////////////////////////////////////////////////////////////
		virtual SOUND_CLASSID	Get_Class_ID (void) const	{ return CLASSID_LISTENER; }

		//////////////////////////////////////////////////////////////////////
		//	Conversion methods
		//////////////////////////////////////////////////////////////////////		
		virtual Listener3DClass *	As_Listener3DClass (void) 	{ return this; }

		//////////////////////////////////////////////////////////////////////
		//	Initialization methods
		//////////////////////////////////////////////////////////////////////				
		virtual void			On_Added_To_Scene (void);
		virtual void			On_Removed_From_Scene (void);

		//////////////////////////////////////////////////////////////////////
		//	State control methods
		//////////////////////////////////////////////////////////////////////
		//virtual bool			Play (void)		{ return false; }
		virtual bool			Pause (void)	{ return false; }
		virtual bool			Resume (void)	{ return false; }
		virtual bool			Stop (bool /*remove*/)		{ return false; }
		virtual void			Seek (unsigned long milliseconds) { }
		virtual SOUND_STATE	Get_State (void) const	{ return STATE_STOPPED; }


		//////////////////////////////////////////////////////////////////////
		//	Attenuation settings
		//////////////////////////////////////////////////////////////////////
		virtual void			Set_Max_Vol_Radius (float radius = 0)			{ }
		virtual float			Get_Max_Vol_Radius (void) const					{ return 0; }
		virtual void			Set_DropOff_Radius (float radius = 1)			{ }
		virtual float			Get_DropOff_Radius (void) const					{ return 0; }

		//////////////////////////////////////////////////////////////////////
		//	Velocity methods
		//////////////////////////////////////////////////////////////////////				
		virtual void			Set_Velocity (const Vector3 &velocity) { }


	protected:

		//////////////////////////////////////////////////////////////////////
		//	Internal representations
		//////////////////////////////////////////////////////////////////////
		virtual void			Start_Sample (void)							{ }
		virtual void			Stop_Sample (void)							{ }
		virtual void			Resume_Sample (void)							{ }
		virtual void			End_Sample (void)								{ }
		virtual void			Set_Sample_Volume (S32 volume)			{ }
		virtual S32				Get_Sample_Volume (void)					{ return 0; }
		virtual void			Set_Sample_Pan (S32 pan)					{ }
		virtual S32				Get_Sample_Pan (void)						{ return 64; }
		virtual void			Set_Sample_Loop_Count (U32 count)		{ }
		virtual U32				Get_Sample_Loop_Count (void)				{ return 0; }
		virtual void			Set_Sample_MS_Position (U32 ms)			{ }
		virtual void			Get_Sample_MS_Position (S32 *len, S32 *pos) { }
		virtual S32				Get_Sample_Playback_Rate (void)			{ return 0; }
		virtual void			Set_Sample_Playback_Rate (S32 rate)		{ }

		//////////////////////////////////////////////////////////////////////
		//	Handle information
		//////////////////////////////////////////////////////////////////////				
		virtual void			Initialize_Miles_Handle (void);
		virtual void			Allocate_Miles_Handle (void);
		virtual void			Free_Miles_Handle (void);

	private:

		//////////////////////////////////////////////////////////////////////
		//	Private member data
		//////////////////////////////////////////////////////////////////////
};


#endif //__LISTENER_H
