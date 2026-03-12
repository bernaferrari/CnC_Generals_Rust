#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __SOUNDSTREAMHANDLE_H
#define __SOUNDSTREAMHANDLE_H


#include "soundhandle.h"


//////////////////////////////////////////////////////////////////////
//
//	SoundStreamHandleClass
//
//////////////////////////////////////////////////////////////////////
class SoundStreamHandleClass	: public SoundHandleClass
{
public:

	///////////////////////////////////////////////////////////////////
	//	Public constructors/destructors
	///////////////////////////////////////////////////////////////////
	SoundStreamHandleClass  (void);
	~SoundStreamHandleClass (void);

	///////////////////////////////////////////////////////////////////
	//	Public methods
	///////////////////////////////////////////////////////////////////
	
	//
	//	RTTI
	//
	SoundStreamHandleClass *	As_SoundStreamHandleClass (void)	{ return this; }

	//
	//	Handle access
	//
	HSAMPLE						Get_HSAMPLE (void)		{ return SampleHandle; }
	HSTREAM						Get_HSTREAM (void)		{ return StreamHandle; }

	//
	//	Inherited
	//
	void							Set_Miles_Handle (uint32 handle);
	void							Initialize (SoundBufferClass *buffer);
	void							Start_Sample (void);
	void							Stop_Sample (void);
	void							Resume_Sample (void);
	void							End_Sample (void);
	void							Set_Sample_Pan (S32 pan);
	S32							Get_Sample_Pan (void);
	void							Set_Sample_Volume (S32 volume);
	S32							Get_Sample_Volume (void);
	void							Set_Sample_Loop_Count (U32 count);
	U32							Get_Sample_Loop_Count (void);
	void							Set_Sample_MS_Position (U32 ms);
	void							Get_Sample_MS_Position (S32 *len, S32 *pos);
	void							Set_Sample_User_Data (S32 i, U32 val);
	U32							Get_Sample_User_Data (S32 i);
	S32							Get_Sample_Playback_Rate (void);
	void							Set_Sample_Playback_Rate (S32 rate);

protected:
	
	///////////////////////////////////////////////////////////////////
	//	Protected methods
	///////////////////////////////////////////////////////////////////
	
	///////////////////////////////////////////////////////////////////
	//	Protected member data
	///////////////////////////////////////////////////////////////////
	HSAMPLE		SampleHandle;
	HSTREAM		StreamHandle;
};


#endif //__SOUNDSTREAMHANDLE_H
