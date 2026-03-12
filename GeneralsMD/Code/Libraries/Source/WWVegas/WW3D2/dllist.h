#if defined(_MSC_VER)
#pragma once
#endif

#ifndef DLLIST_H
#define DLLIST_H


template <class T> class DLNodeClass;

template <class T>
class DLListClass
{
	friend DLNodeClass<T>;
	DLNodeClass<T>* head;
	DLNodeClass<T>* tail;

public:
	DLListClass() : head(0), tail(0) {}
	virtual ~DLListClass() { }

	void Add_Head(DLNodeClass<T>* node);
	void Remove_Head();
	void Add_Tail(DLNodeClass<T>* node);
	void Remove_Tail();

	T* Head() { return static_cast<T*>(head); }
	T* Tail() { return static_cast<T*>(tail); }
	const T* Const_Head() const { return static_cast<const T*>(head); }
	const T* Const_Tail() const { return static_cast<const T*>(tail); }
};

// Destroy-list will call delete for all nodes when the list is destructed. Note that the class doesn't work
// with undeclared pointer types (destructor has to be known).
template <class T>
class DLDestroyListClass : public DLListClass<T>
{
public:
	virtual ~DLDestroyListClass()
	{
		while (T* t=Head()) {		
			delete t;
		}
	}
};

template <class T>
class DLNodeClass : public W3DMPO
{
	// nope, this is an ABC
	//W3DMPO_GLUE(DLNodeClass)

	friend DLListClass<T>;
	DLNodeClass<T>* succ;
	DLNodeClass<T>* pred;
	DLListClass<T>* list;
public:
	DLNodeClass() : succ(0), pred(0), list(0) {}
	~DLNodeClass() { Remove(); }

	void Insert_Before(DLNodeClass<T>* n)
	{
		list=n->list;
		succ=n;
		pred=n->pred;
		if (n->pred) n->pred->succ=this;
		n->pred=this;

		if (list->head==n) {
			list->head=this;
		}
	}

	void Insert_After(DLNodeClass<T>* n)
	{
		list=n->list;
		pred=n;
		succ=n->succ;
		if (n->succ) n->succ->pred=this;
		n->succ=this;

		if (list->tail==n) {
			list->tail=this;
		}
	}

	void Remove()
	{
		if (!list) return;
		if (list->Head()==this) {
			DLListClass<T>* tmp_list=list;
			list=0;
			tmp_list->Remove_Head();
			return;
		}
		if (list->Tail()==this) {
			DLListClass<T>* tmp_list=list;
			list=0;
			tmp_list->Remove_Tail();
			return;
		}
		if (succ) succ->pred=pred;
		if (pred) pred->succ=succ;
		list=0;
	}


	T* Succ() { return static_cast<T*>(succ); }
	T* Pred() { return static_cast<T*>(pred); }
	const T* Const_Succ() const { return static_cast<const T*>(succ); }
	const T* Const_Pred() const { return static_cast<const T*>(pred); }

	DLListClass<T>* List() { return list; }
};


template <class T>
inline void DLListClass<T>::Add_Head(DLNodeClass<T>* n)
{
	n->list=this;
	if (head) {
		n->Insert_Before(head);
		head=n;
	}
	else {
		tail=n;
		head=n;
		n->succ=0;
		n->pred=0;
	}
}

template <class T>
inline void DLListClass<T>::Add_Tail(DLNodeClass<T>* n)
{
	n->list=this;
	if (tail) {
		n->Insert_After(tail);
		tail=n;
	}
	else {
		tail=n;
		head=n;
		n->succ=0;
		n->pred=0;
	}
}

template <class T>
inline void DLListClass<T>::Remove_Head()
{
	if (!head) return;
	DLNodeClass<T>* n=head;
	head=head->Succ();
	if (!head) tail=head;
	else head->pred=0;
	n->Remove();
}

template <class T>
inline void DLListClass<T>::Remove_Tail()
{
	if (!tail) return;
	DLNodeClass<T>* n=tail;
	tail=tail->Pred();
	if (!tail) head=tail;
	else tail->succ=0;
	n->Remove();
}

#endif //DLLIST_H
