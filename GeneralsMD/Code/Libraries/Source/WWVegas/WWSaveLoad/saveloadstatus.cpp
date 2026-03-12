#include "saveloadstatus.h"
#include "mutex.h"

#define MAX_STATUS_TEXT_ID 2

static CriticalSectionClass text_mutex;
static StringClass status_text[MAX_STATUS_TEXT_ID];

void SaveLoadStatus::Set_Status_Text(const char* text,int id)
{
	CriticalSectionClass::LockClass m(text_mutex);
	WWASSERT(id<MAX_STATUS_TEXT_ID);
	status_text[id]=text;
	if (id==0) status_text[1]="";
}

void SaveLoadStatus::Get_Status_Text(StringClass& text, int id)
{
	CriticalSectionClass::LockClass m(text_mutex);
	WWASSERT(id<MAX_STATUS_TEXT_ID);
	text=status_text[id];
}

static	int	status_count;
void	SaveLoadStatus::Reset_Status_Count( void )
{
	status_count = 0;
}

void	SaveLoadStatus::Inc_Status_Count( void )
{
	status_count++;
}

int	SaveLoadStatus::Get_Status_Count( void )
{
	return status_count;
}

