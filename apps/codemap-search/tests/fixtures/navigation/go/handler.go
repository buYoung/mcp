package checkout

import (
	"encoding/json"
	"net/http"
)

type Handler struct {
	service ServicePort
}

func (h Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	decoder := json.NewDecoder(r.Body)
	request := SubmitRequest{}
	if err := decoder.Decode(&request); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		return
	}

	receipt, err := h.service.Submit(r.Context(), request)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		return
	}

	json.NewEncoder(w).Encode(receipt)
}
