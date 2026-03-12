#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __LISTENERHANDLE_H
#define __LISTENERHANDLE_H

#include "sound3dhandle.h"


//////////////////////////////////////////////////////////////////////
//
//	ListenerHandleClass
//
//////////////////////////////////////////////////////////////////////
class ListenerHandleClass : public Sound3DHandleClass
{
public:

	///////////////////////////////////////////////////////////////////
	//	Public constructors/destructors
	///////////////////////////////////////////////////////////////////
	ListenerHandleClass  (void);
	~ListenerHandleClass (void);

	///////////////////////////////////////////////////////////////////
	//	Public methods
	///////////////////////////////////////////////////////////////////

	//
	//	RTTI
	//
	ListenerHandleClass *	As_ListenerHandleClass (void)		{ return this; }

	//
	//	Inherited
	//
	void							Initialize (SoundBufferClass *buffer);
	void							Start_Sample (void)									{ }
	void							Stop_Sample (void)									{ }
	void							Resume_Sample (void)									{ }
	void							End_Sample (void)										{ }
	void							Set_Sample_Volume (S32 volume)					{ }
	S32							Get_Sample_Volume (void)							{ return 0; }
	void							Set_Sample_Pan (S32 pan)							{ }
	S32							Get_Sample_Pan (void)								{ return 64; }
	void							Set_Sample_Loop_Count (U32 count)				{ }
	U32							Get_Sample_Loop_Count (void)						{ return 0; }
	void							Set_Sample_MS_Position (U32 ms)					{ }
	void							Get_Sample_MS_Position (S32 *len, S32 *pos)	{ }
	S32							Get_Sample_Playback_Rate (void)					{ return 0; }
	void							Set_Sample_Playback_Rate (S32 rate)				{ }
	
protected:
	
	///////////////////////////////////////////////////////////////////
	//	Protected methods
	///////////////////////////////////////////////////////////////////
	
	///////////////////////////////////////////////////////////////////
	//	Protected member data
	///////////////////////////////////////////////////////////////////
};


#endif //__LISTENERHANDLE_H
