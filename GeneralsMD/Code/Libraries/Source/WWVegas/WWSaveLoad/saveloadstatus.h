#ifndef WWSAVELOADSTATUS_H
#define WWSAVELOADSTATUS_H

#if defined(_MSC_VER)
#pragma once
#endif

#include "always.h"
#include "wwstring.h"

namespace SaveLoadStatus
{
	void Set_Status_Text(const char* text,int id);

	void	Reset_Status_Count( void );
	void	Inc_Status_Count( void );
	int	Get_Status_Count( void );
	void Get_Status_Text(StringClass& text, int id);
};

#define INIT_STATUS(t) SaveLoadStatus::Set_Status_Text(t,0)
#define INIT_SUB_STATUS(t) SaveLoadStatus::Set_Status_Text(t,1)

#endif
